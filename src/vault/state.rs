use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value as JsonValue;
use vaultrs::api::kv2::responses::{
    ListSecretsResponse, ReadSecretMetadataResponse, ReadSecretResponse, SecretVersionMetadata,
};

#[derive(Debug, Serialize)]
pub(crate) struct EndpointResult<T> {
    pub data: Option<T>,
    pub auth: Option<String>,
    pub lease_id: String,
    pub lease_duration: u32,
    pub renewable: bool,
    pub request_id: String,
    pub warnings: Option<Vec<String>>,
    pub wrap_info: Option<String>,
}

pub(crate) type ReadResponse = EndpointResult<ReadSecretResponse>;
pub(crate) type MetadataResponse = EndpointResult<ReadSecretMetadataResponse>;
pub(crate) type ListResponse = EndpointResult<ListSecretsResponse>;

pub(crate) struct State {
    pub(crate) vault_mount: String,
    pub(crate) list_responses: HashMap<String, ListResponse>,
    pub(crate) metadata_responses: HashMap<String, MetadataResponse>,
    pub(crate) read_responses: HashMap<String, ReadResponse>,
}

impl State {
    pub(crate) fn new(mount: &str) -> Self {
        Self {
            vault_mount: mount.to_string(),
            list_responses: HashMap::new(),
            metadata_responses: HashMap::new(),
            read_responses: HashMap::new(),
        }
    }

    pub(crate) fn vault_mount(&self) -> String {
        self.vault_mount.clone()
    }

    pub(crate) fn get_list_response(&self, p: &str) -> Option<&ListResponse> {
        self.list_responses.get(p)
    }

    pub(crate) fn get_metadata_response(&self, p: &str) -> Option<&MetadataResponse> {
        self.metadata_responses.get(p)
    }

    pub(crate) fn get_read_response(&self, p: &str) -> Option<&ReadResponse> {
        self.read_responses.get(p)
    }

    pub(crate) fn set_list_response(&mut self, p: &str, keys: Vec<String>) {
        let response = new_endpoint_result(ListSecretsResponse { keys });
        self.list_responses.insert(p.to_string(), response);
    }

    pub(crate) fn set_metadata_response(&mut self, p: &str, data: ReadSecretMetadataResponse) {
        let response = new_endpoint_result(data);
        self.metadata_responses.insert(p.to_string(), response);
    }

    pub(crate) fn set_read_response(&mut self, p: &str, data: JsonValue) {
        let metadata = SecretVersionMetadata {
            created_time: "2026-10-18T00:00:00Z".to_string(),
            deletion_time: String::new(),
            custom_metadata: None,
            destroyed: false,
            version: 1,
        };
        let data = ReadSecretResponse { data, metadata };
        let response = new_endpoint_result(data);
        self.read_responses.insert(p.to_string(), response);
    }
}

fn new_endpoint_result<T>(data: T) -> EndpointResult<T> {
    EndpointResult {
        data: Some(data),
        auth: None,
        lease_id: String::new(),
        lease_duration: 0,
        renewable: false,
        request_id: String::from("test-req-id"),
        warnings: None,
        wrap_info: None,
    }
}
