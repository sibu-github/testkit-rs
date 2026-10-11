use bson::{Document, doc};

pub(crate) enum Response {
    Doc(Document),
    Cursor(Vec<Document>),
    Error { code: i32, message: String },
}

impl Response {
    pub fn to_bson_doc(&self, ns: &str) -> Document {
        match self {
            Self::Cursor(docs) => {
                doc! { "cursor": { "id": 0_i64, "ns": ns, "firstBatch": docs }, "ok": 1.0 }
            }
            Self::Doc(doc) => {
                let mut doc = doc.clone();
                doc.insert("ok", 1.0);
                doc
            }
            Self::Error { code, message } => {
                doc! { "ok": 0.0, "errmsg": message, "code": code, "codeName": "UnknownError" }
            }
        }
    }
}

pub fn handshake_command_response() -> Document {
    doc! {
        "helloOk": true,
        "ismaster": true,
        "isWritablePrimary": true,
        "maxBsonObjectSize": 16_777_216,
        "maxMessageSizeBytes": 48_000_000,
        "maxWriteBatchSize": 100_000,
        "localTime": bson::DateTime::now(),
        "logicalSessionTimeoutMinutes": 30,
        "connectionId": 1,
        "minWireVersion": 0,
        "maxWireVersion": 21,
        "ok": 1.0,
    }
}

pub fn default_response() -> Document {
    doc! {"ok": 1.0}
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_default_response() {
        let res = super::default_response();
        assert_eq!(res.get_f64("ok").unwrap(), 1.0);
    }
}
