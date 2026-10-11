use std::sync::{Arc, Mutex};

use tokio::{net::TcpListener, task::JoinHandle};

use crate::mongo::{command_name::CommandName, expectations::*, server::serve, state::State};

pub struct FakeMongo {
    port: u16,
    state: Arc<Mutex<State>>,
    handle: JoinHandle<()>,
}

impl FakeMongo {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(Mutex::new(State::default()));
        let server_state = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            serve(listener, server_state).await;
        });
        Self {
            port,
            state,
            handle,
        }
    }

    pub fn uri(&self) -> String {
        let port = self.port;
        format!("mongodb://127.0.0.1:{port}/?directConnection=true&serverSelectionTimeoutMS=300000")
    }

    pub async fn client(&self) -> mongodb::Client {
        mongodb::Client::with_uri_str(self.uri()).await.unwrap()
    }

    pub async fn db(&self, name: &str) -> mongodb::Database {
        self.client().await.database(name)
    }

    pub fn expect_find(&self) -> FindExpectationBuilder {
        FindExpectationBuilder::new(self.state.clone(), CommandName::Find)
    }

    pub fn expect_find_one(&self) -> FindOneExpectationBuilder {
        FindOneExpectationBuilder::new(self.state.clone(), CommandName::Find)
    }

    pub fn expect_insert_one(&self) -> InsertOneExpectationBuilder {
        InsertOneExpectationBuilder::new(self.state.clone(), CommandName::Insert)
    }

    pub fn expect_insert_many(&self) -> InsertManyExpectationBuilder {
        InsertManyExpectationBuilder::new(self.state.clone(), CommandName::Insert)
    }

    pub fn expect_update_one(&self) -> UpdateOneExpectationBuilder {
        UpdateOneExpectationBuilder::new(self.state.clone(), CommandName::Update)
    }

    pub fn expect_update_many(&self) -> UpdateManyExpectationBuilder {
        UpdateManyExpectationBuilder::new(self.state.clone(), CommandName::Update)
    }

    pub fn expect_delete_one(&self) -> DeleteOneExpectationBuilder {
        DeleteOneExpectationBuilder::new(self.state.clone(), CommandName::Delete)
    }

    pub fn expect_delete_many(&self) -> DeleteManyExpectationBuilder {
        DeleteManyExpectationBuilder::new(self.state.clone(), CommandName::Delete)
    }

    pub fn expect_aggregate(&self) -> AggregateExpectationBuilder {
        AggregateExpectationBuilder::new(self.state.clone(), CommandName::Aggregate)
    }

    pub fn expect_count_documents(&self) -> CountDocumentsExpectationBuilder {
        CountDocumentsExpectationBuilder::new(self.state.clone(), CommandName::Aggregate)
    }

    pub fn expect_distinct(&self) -> DistinctExpectationBuilder {
        DistinctExpectationBuilder::new(self.state.clone(), CommandName::Distinct)
    }

    pub fn expect_find_and_delete(&self) -> FindAndRemoveExpectationBuilder {
        FindAndRemoveExpectationBuilder::new(self.state.clone(), CommandName::FindAndModify)
            .remove()
    }

    pub fn expect_find_and_update(&self) -> FindAndModifyExpectationBuilder {
        FindAndModifyExpectationBuilder::new(self.state.clone(), CommandName::FindAndModify)
    }

    pub fn expect_find_and_replace(&self) -> FindAndModifyExpectationBuilder {
        FindAndModifyExpectationBuilder::new(self.state.clone(), CommandName::FindAndModify)
    }

    pub fn expect_create_index(&self) {
        CreateIndexExpectationBuilder::new(self.state.clone(), CommandName::CreateIndex).respond();
    }
}

impl Drop for FakeMongo {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
