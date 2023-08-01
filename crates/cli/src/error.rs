#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    MatrixSdk(#[from] matrix_sdk::Error),
    #[error(transparent)]
    MatrixSdkClientBuild(#[from] matrix_sdk::ClientBuildError),

    #[error(transparent)]
    RequestChannel(#[from] tokio::sync::mpsc::error::SendError<crate::ImageRequest>),
    #[error(transparent)]
    ResponseChannel(#[from] tokio::sync::mpsc::error::SendError<crate::ImageResult>),
}

pub type Result<T> = std::result::Result<T, Error>;
