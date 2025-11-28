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

#[macro_export]
macro_rules! token_list {
    [$($t:expr),* $(,)?] => {
        {
            let mut v = Vec::new();
            $(
                v.push($t.to_string());
            )*
            v
        }
    };
}

pub(crate) use check_required;
