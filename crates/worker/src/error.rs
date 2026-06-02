use worker::Response;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Forbidden,
    Internal(String),
}

impl AppError {
    pub fn into_response(self) -> worker::Result<Response> {
        match self {
            Self::NotFound(m) => Response::error(m, 404),
            Self::BadRequest(m) => Response::error(m, 400),
            Self::Forbidden => Response::error("Forbidden", 403),
            Self::Internal(m) => {
                // No request handle here, so the id is "unknown"; the http.out
                // line for this request carries the real rid alongside.
                crate::observability::log_error("unknown", "AppError::Internal", &m);
                Response::error("Internal error", 500)
            }
        }
    }
}
