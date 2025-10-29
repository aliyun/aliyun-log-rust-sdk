
macro_rules! check_required {
    ($(($name:expr, $value:expr)),* $(,)?) => {
        {
            $(
                if $value.is_none() {
                    return Err(crate::RequestError::from(
                        crate::RequestErrorKind::MissingRequiredParameter($name.to_string()),
                    ));
                }
            )*
        }
    };
}

pub(crate) use check_required;
