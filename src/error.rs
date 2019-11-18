error_chain! {
    foreign_links {
        Fmt(::std::fmt::Error);
        Io(::std::io::Error);
        RecvError(::tokio::sync::oneshot::error::RecvError);
        SendError(::tokio::sync::mpsc::error::UnboundedSendError);
    }

    errors {
        MemcacheError(errorCode: u16) {
            description("An error was returned by the server")
            display("Error code: '{}'", errorCode)
        }

        ClientError(error: String) {
            description("Client error")
            display("Client error: {}", error)
        }

        UnknownError(error: String) {
            description("Unknown error")
            display("Unknown error: {}", error)
        }
    }
}