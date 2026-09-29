use aliyun_log_rust_sdk::Error;

/// Java producer's explicit error codes, with retries for unrecognized errors.
/// HTTP status deliberately does not participate. The sender separately enforces
/// the attempt limit and delivery deadline.
pub(super) fn retryable(error: &Error) -> bool {
    match error {
        Error::InvalidConfig(_) | Error::RequestPreparation(_) => false,
        Error::Server { error_code, .. } => match error_code.as_str() {
            // RetriableErrors.java and SendProducerBatchTask.isRetriableException:
            // https://github.com/aliyun/aliyun-log-java-producer/tree/093b5cd73c16bc18444f0c7dd26212f800cac6bc
            // SignatureNotMatch also appears in Errors.java; the actual Java
            // retry decision takes precedence over that file's comment.
            "RequestError"
            | "Unauthorized"
            | "WriteQuotaExceed"
            | "ShardWriteQuotaExceed"
            | "ExceedQuota"
            | "InternalServerError"
            | "ServerBusy"
            | "BadResponse"
            | "ProjectNotExists"
            | "LogstoreNotExists"
            | "SocketTimeout"
            | "SignatureNotMatch" => true,
            // Explicit non-retriable codes from Java's Errors.java.
            "ProjectConfigNotExist"
            | "ProjectNotExist"
            | "MissAccessKeyId"
            | "RequestTimeTooSkewed"
            | "ProducerException" => false,
            // Unlike Java's closed whitelist, unknown server codes are retried.
            _ => true,
        },
        // Includes network, response parsing, credentials and unclassified errors.
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_determine_retries_independently_of_http_status() {
        let cases = [
            ("RequestError", true),
            ("Unauthorized", true),
            ("WriteQuotaExceed", true),
            ("ShardWriteQuotaExceed", true),
            ("ExceedQuota", true),
            ("InternalServerError", true),
            ("ServerBusy", true),
            ("BadResponse", true),
            ("ProjectNotExists", true),
            ("LogstoreNotExists", true),
            ("SocketTimeout", true),
            ("SignatureNotMatch", true),
            ("ProjectConfigNotExist", false),
            ("ProjectNotExist", false),
            ("MissAccessKeyId", false),
            ("RequestTimeTooSkewed", false),
            ("ProducerException", false),
            ("FutureServiceError", true),
            ("", true),
        ];
        for (code, expected) in cases {
            for status in [400, 401, 403, 404, 413, 429, 500, 501, 503, 505] {
                let error = Error::Server {
                    error_code: code.into(),
                    error_message: "test".into(),
                    http_status: status,
                    request_id: None,
                };
                assert_eq!(retryable(&error), expected, "{code} / HTTP {status}");
            }
        }
    }
}
