use std::sync::{Arc, Mutex};

use tokio::{net::TcpListener, task::JoinHandle};

use crate::mongo::{command_name::CommandName, expectations::*, server::serve, state::State};

/// An in-process MongoDB wire-protocol server for tests.
///
/// `FakeMongo` listens on a random local port and works with the official
/// [`mongodb::Client`]. Configure expected operations with the `expect_*`
/// methods, then use [`FakeMongo::client`], [`FakeMongo::db`], or
/// [`FakeMongo::uri`] to connect the code under test.
///
/// The server runs in a background Tokio task and shuts down when the
/// `FakeMongo` value is dropped. It does not require a MongoDB installation.
pub struct FakeMongo {
    port: u16,
    state: Arc<Mutex<State>>,
    handle: JoinHandle<()>,
}

impl FakeMongo {
    /// Starts a fake MongoDB server on a random available localhost port.
    ///
    /// The server runs in a background Tokio task until this value is dropped.
    ///
    /// # Panics
    ///
    /// Panics if the server cannot bind to a local port or inspect its bound
    /// address.
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

    /// Returns the MongoDB connection URI for this server.
    ///
    /// The URI enables direct connection mode so the driver does not attempt
    /// topology discovery through another MongoDB node.
    pub fn uri(&self) -> String {
        let port = self.port;
        format!("mongodb://127.0.0.1:{port}/?directConnection=true&serverSelectionTimeoutMS=300000")
    }

    /// Creates an official MongoDB client configured to connect to this server.
    ///
    /// # Panics
    ///
    /// Panics if the generated connection URI cannot be converted into a
    /// [`mongodb::Client`].
    pub async fn client(&self) -> mongodb::Client {
        mongodb::Client::with_uri_str(self.uri()).await.unwrap()
    }

    /// Creates a MongoDB database handle with the given `name`.
    ///
    /// This is a convenience wrapper around [`FakeMongo::client`] and
    /// [`mongodb::Client::database`].
    pub async fn db(&self, name: &str) -> mongodb::Database {
        self.client().await.database(name)
    }

    /// Begins configuring an expected `find` command that may return multiple
    /// documents.
    pub fn expect_find(&self) -> FindExpectationBuilder {
        FindExpectationBuilder::new(self.state.clone(), CommandName::Find)
    }

    /// Begins configuring an expected `find` command that returns at most one
    /// document.
    pub fn expect_find_one(&self) -> FindOneExpectationBuilder {
        FindOneExpectationBuilder::new(self.state.clone(), CommandName::Find)
    }

    /// Begins configuring an expected single-document `insert` command.
    pub fn expect_insert_one(&self) -> InsertOneExpectationBuilder {
        InsertOneExpectationBuilder::new(self.state.clone(), CommandName::Insert)
    }

    /// Begins configuring an expected multi-document `insert` command.
    pub fn expect_insert_many(&self) -> InsertManyExpectationBuilder {
        InsertManyExpectationBuilder::new(self.state.clone(), CommandName::Insert)
    }

    /// Begins configuring an expected single-document `update` command.
    pub fn expect_update_one(&self) -> UpdateOneExpectationBuilder {
        UpdateOneExpectationBuilder::new(self.state.clone(), CommandName::Update)
    }

    /// Begins configuring an expected multi-document `update` command.
    pub fn expect_update_many(&self) -> UpdateManyExpectationBuilder {
        UpdateManyExpectationBuilder::new(self.state.clone(), CommandName::Update)
    }

    /// Begins configuring an expected single-document `delete` command.
    pub fn expect_delete_one(&self) -> DeleteOneExpectationBuilder {
        DeleteOneExpectationBuilder::new(self.state.clone(), CommandName::Delete)
    }

    /// Begins configuring an expected multi-document `delete` command.
    pub fn expect_delete_many(&self) -> DeleteManyExpectationBuilder {
        DeleteManyExpectationBuilder::new(self.state.clone(), CommandName::Delete)
    }

    /// Begins configuring an expected `aggregate` command.
    pub fn expect_aggregate(&self) -> AggregateExpectationBuilder {
        AggregateExpectationBuilder::new(self.state.clone(), CommandName::Aggregate)
    }

    /// Begins configuring an expected `count_documents` operation.
    ///
    /// The MongoDB driver implements this operation with an `aggregate`
    /// command, which the fake validates and answers.
    pub fn expect_count_documents(&self) -> CountDocumentsExpectationBuilder {
        CountDocumentsExpectationBuilder::new(self.state.clone(), CommandName::Aggregate)
    }

    /// Begins configuring an expected `distinct` command.
    pub fn expect_distinct(&self) -> DistinctExpectationBuilder {
        DistinctExpectationBuilder::new(self.state.clone(), CommandName::Distinct)
    }

    /// Begins configuring an expected `findOneAndDelete` operation.
    ///
    /// The MongoDB driver sends this operation as a `findAndModify` command
    /// with its remove flag enabled.
    pub fn expect_find_and_delete(&self) -> FindAndRemoveExpectationBuilder {
        FindAndRemoveExpectationBuilder::new(self.state.clone(), CommandName::FindAndModify)
            .remove()
    }

    /// Begins configuring an expected `findOneAndUpdate` operation.
    pub fn expect_find_and_update(&self) -> FindAndModifyExpectationBuilder {
        FindAndModifyExpectationBuilder::new(self.state.clone(), CommandName::FindAndModify)
    }

    /// Begins configuring an expected `findOneAndReplace` operation.
    pub fn expect_find_and_replace(&self) -> FindAndModifyExpectationBuilder {
        FindAndModifyExpectationBuilder::new(self.state.clone(), CommandName::FindAndModify)
    }

    /// Registers a successful response for the next `createIndexes` command.
    pub fn expect_create_index(&self) {
        CreateIndexExpectationBuilder::new(self.state.clone(), CommandName::CreateIndex).respond();
    }
}

impl Drop for FakeMongo {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
