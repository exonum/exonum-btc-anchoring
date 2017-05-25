#[macro_export]
macro_rules! request {
    (
        method: $method:expr,
        params: [$($params:tt)+]
    ) => {
        Request {
            method: $method,
            params: json!([$($params)+]).as_array().unwrap().clone(),
            response: Ok(::serde_json::Value::Null)
        }
    };
    (
        method: $method:expr,
        params: [$($params:tt)+],
        response: $($response:tt)+
    ) => {
        Request {
            method: $method,
            params: json!([$($params)+]).as_array().unwrap().clone(),
            response: Ok(json!($($response)+)),
        }
    };
    (
        method: $method:expr,
        params: [$($params:tt)+],
        error: $($err:tt)+
    ) => {
        Request {
            method: $method,
            params: json!([$($params)+]).as_array().unwrap().clone(),
            response: Err($($err)+)
        }
    };
}
