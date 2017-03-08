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

/// Copyed from ::serde_json 0.9. TODO move our code to serve 0.9
#[macro_export]
macro_rules! json {
    // Hide distracting implementation details from the generated rustdoc.
    ($($json:tt)+) => {
        json_internal!($($json)+)
    };
}

// Rocket relies on this because they export their own `json!` with a different
// doc comment than ours, and various Rust bugs prevent them from calling our
// `json!` from their `json!` so they call `json_internal!` directly. Check with
// @SergioBenitez before making breaking changes to this macro.
//
// Changes are fine as long as `json_internal!` does not call any new helper
// macros and can still be invoked as `json_internal!($($json)+)`.
#[macro_export]
#[doc(hidden)]
macro_rules! json_internal {
    //////////////////////////////////////////////////////////////////////////
    // TT muncher for parsing the inside of an array [...]. Produces a vec![...]
    // of the elements.
    //
    // Must be invoked as: json_internal!(@array [] $($tt)*)
    //////////////////////////////////////////////////////////////////////////

    // Done with trailing comma.
    (@array [$($elems:expr,)*]) => {
        vec![$($elems,)*]
    };

    // Done without trailing comma.
    (@array [$($elems:expr),*]) => {
        vec![$($elems),*]
    };

    // Next element is `null`.
    (@array [$($elems:expr,)*] null $($rest:tt)*) => {
        json_internal!(@array [$($elems,)* json_internal!(null)] $($rest)*)
    };

    // Next element is `true`.
    (@array [$($elems:expr,)*] true $($rest:tt)*) => {
        json_internal!(@array [$($elems,)* json_internal!(true)] $($rest)*)
    };

    // Next element is `false`.
    (@array [$($elems:expr,)*] false $($rest:tt)*) => {
        json_internal!(@array [$($elems,)* json_internal!(false)] $($rest)*)
    };

    // Next element is an array.
    (@array [$($elems:expr,)*] [$($array:tt)*] $($rest:tt)*) => {
        json_internal!(@array [$($elems,)* json_internal!([$($array)*])] $($rest)*)
    };

    // Next element is a map.
    (@array [$($elems:expr,)*] {$($map:tt)*} $($rest:tt)*) => {
        json_internal!(@array [$($elems,)* json_internal!({$($map)*})] $($rest)*)
    };

    // Next element is an expression followed by comma.
    (@array [$($elems:expr,)*] $next:expr, $($rest:tt)*) => {
        json_internal!(@array [$($elems,)* json_internal!($next),] $($rest)*)
    };

    // Last element is an expression with no trailing comma.
    (@array [$($elems:expr,)*] $last:expr) => {
        json_internal!(@array [$($elems,)* json_internal!($last)])
    };

    // Comma after the most recent element.
    (@array [$($elems:expr),*] , $($rest:tt)*) => {
        json_internal!(@array [$($elems,)*] $($rest)*)
    };

    //////////////////////////////////////////////////////////////////////////
    // TT muncher for parsing the inside of an object {...}. Each entry is
    // inserted into the given map variable.
    //
    // Must be invoked as: json_internal!(@object $map () $($tt)*)
    //////////////////////////////////////////////////////////////////////////

    // Done.
    (@object $object:ident ()) => {};

    // Insert the current entry followed by trailing comma.
    (@object $object:ident [$($key:tt)+] ($value:expr) , $($rest:tt)*) => {
        $object.insert(($($key)+).into(), $value);
        json_internal!(@object $object () $($rest)*);
    };

    // Insert the last entry without trailing comma.
    (@object $object:ident [$($key:tt)+] ($value:expr)) => {
        $object.insert(($($key)+).into(), $value);
    };

    // Next value is `null`.
    (@object $object:ident ($($key:tt)+) : null $($rest:tt)*) => {
        json_internal!(@object $object [$($key)+] (json_internal!(null)) $($rest)*);
    };

    // Next value is `true`.
    (@object $object:ident ($($key:tt)+) : true $($rest:tt)*) => {
        json_internal!(@object $object [$($key)+] (json_internal!(true)) $($rest)*);
    };

    // Next value is `false`.
    (@object $object:ident ($($key:tt)+) : false $($rest:tt)*) => {
        json_internal!(@object $object [$($key)+] (json_internal!(false)) $($rest)*);
    };

    // Next value is an array.
    (@object $object:ident ($($key:tt)+) : [$($array:tt)*] $($rest:tt)*) => {
        json_internal!(@object $object [$($key)+] (json_internal!([$($array)*])) $($rest)*);
    };

    // Next value is a map.
    (@object $object:ident ($($key:tt)+) : {$($map:tt)*} $($rest:tt)*) => {
        json_internal!(@object $object [$($key)+] (json_internal!({$($map)*})) $($rest)*);
    };

    // Next value is an expression followed by comma.
    (@object $object:ident ($($key:tt)+) : $value:expr , $($rest:tt)*) => {
        json_internal!(@object $object [$($key)+] (json_internal!($value)) , $($rest)*);
    };

    // Last value is an expression with no trailing comma.
    (@object $object:ident ($($key:tt)+) : $value:expr) => {
        json_internal!(@object $object [$($key)+] (json_internal!($value)));
    };

    // Missing value for last entry. Trigger a reasonable error message
    // referring to the unexpected end of macro invocation.
    (@object $object:ident ($($key:tt)+) :) => {
        json_internal!();
    };

    // Misplaced colon. Trigger a reasonable error message by failing to match
    // the colon in the recursive call.
    (@object $object:ident () : $($rest:tt)*) => {
        json_internal!(:);
    };

    // Found a comma inside a key. Trigger a reasonable error message by failing
    // to match the comma in the recursive call.
    (@object $object:ident ($($key:tt)*) , $($rest:tt)*) => {
        json_internal!(,);
    };

    // Key is fully parenthesized. This avoids clippy double_parens false
    // positives because the parenthesization may be necessary here.
    (@object $object:ident () ($key:expr) : $($rest:tt)*) => {
        json_internal!(@object $object ($key) : $($rest)*);
    };

    // Munch a token into the current key.
    (@object $object:ident ($($key:tt)*) $tt:tt $($rest:tt)*) => {
        json_internal!(@object $object ($($key)* $tt) $($rest)*);
    };

    //////////////////////////////////////////////////////////////////////////
    // The main implementation.
    //
    // Must be invoked as: json_internal!($($json)+)
    //////////////////////////////////////////////////////////////////////////

    (null) => {
        ::serde_json::Value::Null
    };

    (true) => {
        ::serde_json::Value::Bool(true)
    };

    (false) => {
        ::serde_json::Value::Bool(false)
    };

    ([]) => {
        ::serde_json::Value::Array(vec![])
    };

    ([ $($tt:tt)+ ]) => {
        ::serde_json::Value::Array(json_internal!(@array [] $($tt)+))
    };

    ({}) => {
        ::serde_json::Value::Object(::serde_json::Map::new())
    };

    ({ $($tt:tt)+ }) => {
        ::serde_json::Value::Object({
            let mut object = ::serde_json::Map::new();
            json_internal!(@object object () $($tt)+);
            object
        })
    };

    // Any Serialize type: numbers, strings, struct literals, variables etc.
    // Must be below every other rule.
    ($other:expr) => {
        ::serde_json::value::ToJson::to_json(&$other)
    };
}
