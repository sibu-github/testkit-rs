use std::collections::HashMap;

use hyper::body::Bytes;

#[derive(Default)]
pub(crate) struct State {
    pub(crate) put_objects: Vec<String>,
    pub(crate) get_objects: HashMap<String, Bytes>,
}
