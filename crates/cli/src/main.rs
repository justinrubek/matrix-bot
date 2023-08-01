use crate::{
    commands::{Commands, MatrixBotCommands},
    error::Result,
};
use clap::Parser;
use matrix_sdk::{
    config::SyncSettings,
    event_handler::Ctx,
    room::Room,
    ruma::{
        events::{
            room::message::{MessageType, RoomMessageEventContent, SyncRoomMessageEvent},
            MessageLikeEvent, OriginalMessageLikeEvent, SyncMessageLikeEvent,
        },
        OwnedRoomId, OwnedUserId,
    },
    Client,
};
use tracing::info;

mod commands;
mod error;

#[derive(Clone, Debug)]
pub struct ImageRequest {
    pub prompt: String,

    pub message_context: MessageContext,
}

#[derive(Clone, Debug)]
pub struct ImageResult {
    pub prompt: String,
    pub image: Vec<u8>,

    pub message_context: MessageContext,
}

#[derive(Clone, Debug)]
pub struct MessageContext {
    pub room_id: OwnedRoomId,
    pub event: OriginalMessageLikeEvent<RoomMessageEventContent>,
}

#[derive(Clone, Debug)]
pub struct HandlerContext {
    /// The channel to send requests to the image generation thread
    pub tx_request: tokio::sync::mpsc::UnboundedSender<ImageRequest>,

    /// the id of the current user
    pub user_id: OwnedUserId,
}

impl HandlerContext {
    pub fn new(
        tx_request: tokio::sync::mpsc::UnboundedSender<ImageRequest>,
        user_id: OwnedUserId,
    ) -> Self {
        Self {
            tx_request,
            user_id,
        }
    }

    pub fn send_request(&self, request: ImageRequest) -> Result<()> {
        self.tx_request.send(request)?;

        Ok(())
    }
}

async fn on_room_message(event: SyncRoomMessageEvent, room: Room, ctx: Ctx<HandlerContext>) {
    if let Room::Joined(joined) = room {
        if let SyncMessageLikeEvent::Original(m) = event.clone() {
            let sender = m.sender;

            if sender == ctx.0.user_id {
                return;
            }

            if let MessageType::Text(t) = m.content.msgtype {
                let body = t.body;

                let room_id = joined.room_id().to_owned();
                let event = event.into_full_event(room_id.clone());
                if let MessageLikeEvent::Original(event) = event {
                    info!("{sender}: {body}");
                    ctx.send_request(ImageRequest {
                        prompt: body,
                        message_context: MessageContext {
                            room_id: room_id.clone(),
                            event,
                        },
                    })
                    .expect("failed to send request");
                }
            }
        }
    }
}

async fn login_and_sync(homeserver: &str, user: &str, password: &str) -> Result<(Client, String)> {
    let client = Client::builder().homeserver_url(homeserver).build().await?;

    // First we need to log in.
    client
        .login_username(user, password)
        .initial_device_display_name("generation-bot")
        .send()
        .await?;

    info!("logged in as {user}");
    // An initial sync to set up state and so our bot doesn't respond to old
    // messages.
    let response = client.sync_once(SyncSettings::default()).await.unwrap();

    Ok((client, response.next_batch))
}

/// Sets up the event handler and sync loop
async fn setup_event_handler(client: Client, token: String, context: HandlerContext) -> Result<()> {
    // add our CommandBot to be notified of incoming messages, we do this after the
    // initial sync to avoid responding to messages before the bot was running.
    client.add_event_handler_context(context);
    client.add_event_handler(on_room_message);

    // since we called `sync_once` before we entered our sync loop we must pass
    // that sync token to `sync`
    let settings = SyncSettings::default().token(token);
    // this keeps state from the server streaming in to CommandBot via the
    // EventHandler trait
    client.sync(settings).await?;

    Ok(())
}

fn generate_images_from_requests(
    mut message_rx: tokio::sync::mpsc::UnboundedReceiver<ImageRequest>,
    response_tx: tokio::sync::mpsc::UnboundedSender<ImageResult>,
) -> Result<()> {
    while let Some(request) = message_rx.blocking_recv() {
        // TODO: generate image
        // for now, just send back the prompt and an empty image
        response_tx.send(ImageResult {
            prompt: request.prompt,
            image: vec![],
            message_context: request.message_context,
        })?;
    }

    Ok(())
}

/// Receives responses asynchronously and sends them back to the room they came from
async fn send_responses(
    client: Client,
    mut response_rx: tokio::sync::mpsc::UnboundedReceiver<ImageResult>,
) -> Result<()> {
    while let Some(response) = response_rx.recv().await {
        let body = format!("Okay. Here is `{}`", response.prompt);
        let content = RoomMessageEventContent::text_plain(body)
            .make_reply_to(&response.message_context.event);

        let room = client
            .get_joined_room(&response.message_context.room_id)
            .unwrap();

        room.send(content, None).await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // create two channels for communicating between the synchronous thread and the async runtime
    // for communicating image requests from messages
    let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel::<ImageRequest>();
    // for sending response images back to the message handler
    let (response_tx, response_rx) = tokio::sync::mpsc::unbounded_channel::<ImageResult>();

    // Spawn a long-running thread to process requests synchronously
    std::thread::spawn(move || {
        generate_images_from_requests(message_rx, response_tx).unwrap();
    });

    let homeserver = std::env::var("MATRIX_HOMESERVER").unwrap();
    let user = std::env::var("MATRIX_USERNAME").unwrap();
    let password = std::env::var("MATRIX_PASSWORD").unwrap();

    let args = commands::Args::parse();
    match args.command {
        Commands::MatrixBot(matrix_bot) => {
            let cmd = matrix_bot.command;
            match cmd {
                MatrixBotCommands::Run(_run_args) => {
                    let (client, token) = login_and_sync(&homeserver, &user, &password).await?;
                    let client_clone = client.clone();

                    // create a context to pass to our event handler
                    let user_id = client.user_id().expect("user id not found").to_owned();
                    let context = HandlerContext::new(message_tx, user_id);

                    let event_handler =
                        tokio::spawn(setup_event_handler(client_clone, token, context));
                    let response_proxy = tokio::spawn(send_responses(client, response_rx));

                    let (res_event, res_proxy) = tokio::join!(event_handler, response_proxy);
                    if let Err(e) = res_event {
                        eprintln!("event handler failed: {}", e);
                    }

                    if let Err(e) = res_proxy {
                        eprintln!("response proxy failed: {}", e);
                    }
                }
            }
        }
    }

    Ok(())
}
