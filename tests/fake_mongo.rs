#![cfg(feature = "mongo")]

use anyhow::anyhow;
use bson::{Bson, Document, doc};
use futures::TryStreamExt;
use mongodb::{
    Client, IndexModel,
    options::ReturnDocument,
    results::{InsertManyResult, InsertOneResult, UpdateResult},
};
use serde::{Deserialize, Serialize};
use testkit_rs::mongo::FakeMongo;

const DB_NAME: &str = "testdb";
const COLLECTION: &str = "users";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
struct User {
    id: i32,
    name: String,
}

impl User {
    fn new(id: i32) -> Self {
        Self {
            id,
            name: String::new(),
        }
    }
}

async fn find_user_by_id(client: &Client, id: i32) -> anyhow::Result<User> {
    let filter = doc! {"id": id};
    client
        .database(DB_NAME)
        .collection::<User>(COLLECTION)
        .find_one(filter)
        .await?
        .ok_or(anyhow!("user not found"))
}

async fn find_users(client: &Client) -> anyhow::Result<Vec<User>> {
    let filter = doc! {"id": {"$gte": 1}};
    let sort = doc! {"id": 1};
    let project = doc! {"_id": 0};
    let limit = 10;
    let skip = 100;
    let res = client
        .database(DB_NAME)
        .collection::<User>(COLLECTION)
        .find(filter)
        .sort(sort)
        .projection(project)
        .limit(limit)
        .skip(skip)
        .await?
        .try_collect()
        .await?;
    Ok(res)
}

async fn insert_user(client: &Client, user: &User) -> anyhow::Result<InsertOneResult> {
    let coll = client.database(DB_NAME).collection::<User>(COLLECTION);
    let res = coll.insert_one(user).await?;
    Ok(res)
}

async fn insert_users(client: &Client, users: &[User]) -> anyhow::Result<InsertManyResult> {
    let coll = client.database(DB_NAME).collection::<User>(COLLECTION);
    let res = coll.insert_many(users).await?;
    Ok(res)
}

async fn update_user(client: &Client, upsert: bool) -> anyhow::Result<UpdateResult> {
    let filter = doc! { "id": 1 };
    let update = doc! { "$set": { "name": "siba" } };
    let coll = client.database(DB_NAME).collection::<User>(COLLECTION);
    let res = coll.update_one(filter, update).upsert(upsert).await?;
    Ok(res)
}

async fn update_users(client: &Client) -> anyhow::Result<UpdateResult> {
    let filter = doc! {};
    let update = doc! { "$set": { "users.$[user].name": "siba" } };
    let array_filters = [doc! {"user.id": { "$gte": 1 }}];
    let coll = client.database(DB_NAME).collection::<User>(COLLECTION);
    let res = coll
        .update_many(filter, update)
        .array_filters(array_filters)
        .await?;
    Ok(res)
}

#[tokio::test]
async fn find_one_returns_expected_documents() {
    let expected_id = 23;
    let fake = FakeMongo::start().await;
    fake.expect_find_one()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(doc! {"id": expected_id})
        .respond_with(&User::new(expected_id));
    let client = fake.client().await;
    let user = find_user_by_id(&client, 23).await.unwrap();
    assert_eq!(user.id, expected_id);
}

#[tokio::test]
async fn find_one_returns_failure() {
    let fake = FakeMongo::start().await;
    fake.expect_find_one().fail("custom error");
    let client = fake.client().await;
    let user = find_user_by_id(&client, 1).await;
    assert!(user.is_err());
}

#[tokio::test]
async fn find_returns_expected_documents() {
    let users = vec![User::new(1), User::new(2)];
    let fake = FakeMongo::start().await;
    fake.expect_find()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(doc! {"id": {"$gte": 1}})
        .sort(doc! {"id": 1})
        .project(doc! {"_id": 0})
        .limit(10)
        .skip(100)
        .respond_with(&users);
    let client = fake.client().await;
    let res = find_users(&client).await.unwrap();
    assert_eq!(res, users);
}

#[tokio::test]
async fn find_returns_failure() {
    let fake = FakeMongo::start().await;
    fake.expect_find().fail("custom error");
    let client = fake.client().await;
    let res = find_users(&client).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn insert_one_inserts_expected_document() {
    let user = User::new(1);
    let fake = FakeMongo::start().await;
    fake.expect_insert_one()
        .database(DB_NAME)
        .collection(COLLECTION)
        .with_document(&user)
        .respond();
    let client = fake.client().await;
    let res = insert_user(&client, &user).await.unwrap();
    assert!(matches!(res.inserted_id, Bson::ObjectId(_)));
}

#[tokio::test]
async fn insert_one_returns_failure() {
    let user = User::new(1);
    let fake = FakeMongo::start().await;
    fake.expect_insert_one().fail("custom error");
    let client = fake.client().await;
    let res = insert_user(&client, &user).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn insert_many_inserts_expected_documents() {
    let users = vec![User::new(1), User::new(2)];
    let fake = FakeMongo::start().await;
    fake.expect_insert_many()
        .database(DB_NAME)
        .collection(COLLECTION)
        .ordered(true)
        .with_documents(&users)
        .respond_with(users.len() as u32);
    let client = fake.client().await;
    let res = insert_users(&client, &users).await.unwrap();
    assert_eq!(res.inserted_ids.len(), 2);
}

#[tokio::test]
async fn insert_many_returns_failure() {
    let users = vec![User::new(1), User::new(2)];
    let fake = FakeMongo::start().await;
    fake.expect_insert_many().fail("custom error");
    let client = fake.client().await;
    let res = insert_users(&client, &users).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn update_one_updates_expected_document() {
    let fake = FakeMongo::start().await;
    fake.expect_update_one()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(doc! { "id": 1 })
        .update(doc! { "$set": { "name": "siba" } })
        .respond();
    let client = fake.client().await;
    let res = update_user(&client, false).await.unwrap();
    assert_eq!(res.matched_count, 1);
    assert_eq!(res.modified_count, 1);
}

#[tokio::test]
async fn update_one_upsert_document() {
    let fake = FakeMongo::start().await;
    fake.expect_update_one()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(doc! { "id": 1 })
        .update(doc! { "$set": { "name": "siba" } })
        .upsert(true)
        .respond_upserted();
    let client = fake.client().await;
    let res = update_user(&client, true).await.unwrap();
    assert_eq!(res.matched_count, 0);
    assert_eq!(res.modified_count, 0);
    assert!(res.upserted_id.is_some());
}

#[tokio::test]
async fn update_one_returns_failure() {
    let fake = FakeMongo::start().await;
    fake.expect_update_one().fail("custom error");
    let client = fake.client().await;
    let res = update_user(&client, false).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn update_many_updates_expected_document() {
    let fake = FakeMongo::start().await;
    fake.expect_update_many()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(doc! {})
        .update(doc! { "$set": { "users.$[user].name": "siba" } })
        .array_filters([doc! {"user.id": { "$gte": 1 }}])
        .respond_with(2, 2);
    let client = fake.client().await;
    let res = update_users(&client).await.unwrap();
    assert_eq!(res.matched_count, 2);
    assert_eq!(res.modified_count, 2);
}

#[tokio::test]
async fn update_many_returns_failure() {
    let fake = FakeMongo::start().await;
    fake.expect_update_many().fail("custom error");
    let client = fake.client().await;
    let res = update_users(&client).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn update_many_upsert_document() {
    let fake = FakeMongo::start().await;
    fake.expect_update_many()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(doc! { "id": 1 })
        .update(doc! { "$set": { "name": "siba" } })
        .upsert(true)
        .respond_upserted(0, 0);
    let res = fake
        .db(DB_NAME)
        .await
        .collection::<User>(COLLECTION)
        .update_many(doc! { "id": 1 }, doc! { "$set": { "name": "siba" } })
        .upsert(true)
        .await
        .unwrap();
    assert_eq!(res.matched_count, 0);
    assert_eq!(res.modified_count, 0);
    assert!(res.upserted_id.is_some());
}

#[tokio::test]
async fn aggregate_returns_expected_document() {
    let users = vec![doc! {"id": 1}, doc! {"id": 2}];
    let filter = doc! { "$match": { "id": { "$gte": 1}}};
    let project = doc! { "$project": { "_id": 0, "id": 1}};
    let pipeline = vec![filter, project];
    let fake = FakeMongo::start().await;
    fake.expect_aggregate()
        .database(DB_NAME)
        .collection(COLLECTION)
        .pipeline(pipeline.clone())
        .respond_with(users.clone());
    let coll = fake.db(DB_NAME).await.collection::<Document>(COLLECTION);
    let res = coll
        .aggregate(pipeline.clone())
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(res, users);
}

#[tokio::test]
async fn count_documents_expected_result() {
    let filter = doc! {"age": {"$gte": 30}};
    let fake = FakeMongo::start().await;
    fake.expect_count_documents()
        .database(DB_NAME)
        .collection(COLLECTION)
        .query(filter.clone())
        .respond_with(3);
    let coll = fake.db(DB_NAME).await.collection::<Document>(COLLECTION);
    let count = coll.count_documents(filter).await.unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn delete_one_returns_expected_result() {
    let filter = doc! {"age": {"$gte": 30}};
    let fake = FakeMongo::start().await;
    fake.expect_delete_one()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(filter.clone())
        .respond(0);
    let coll = fake.db(DB_NAME).await.collection::<Document>(COLLECTION);
    let res = coll.delete_one(filter.clone()).await.unwrap();
    assert_eq!(res.deleted_count, 0);
    fake.expect_delete_one()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(filter.clone())
        .respond(1);
    let coll = fake.db(DB_NAME).await.collection::<Document>(COLLECTION);
    let res = coll.delete_one(filter).await.unwrap();
    assert_eq!(res.deleted_count, 1);
}

#[tokio::test]
async fn delete_many_returns_expected_result() {
    let filter = doc! {"age": {"$gte": 30}};
    let fake = FakeMongo::start().await;
    fake.expect_delete_many()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(filter.clone())
        .respond(0);
    let coll = fake.db(DB_NAME).await.collection::<Document>(COLLECTION);
    let res = coll.delete_many(filter.clone()).await.unwrap();
    assert_eq!(res.deleted_count, 0);
    fake.expect_delete_many()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(filter.clone())
        .respond(3);
    let coll = fake.db(DB_NAME).await.collection::<Document>(COLLECTION);
    let res = coll.delete_many(filter).await.unwrap();
    assert_eq!(res.deleted_count, 3);
}

#[tokio::test]
async fn distinct_expected_result() {
    let filter = doc! {"age": {"$gte": 30}};
    let fake = FakeMongo::start().await;
    fake.expect_distinct()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(filter.clone())
        .key("name")
        .respond_with(["a", "b", "c"]);
    let coll = fake.db(DB_NAME).await.collection::<Document>(COLLECTION);
    let res = coll.distinct("name", filter.clone()).await.unwrap();
    assert_eq!(
        res,
        vec![
            bson::Bson::String("a".to_string()),
            bson::Bson::String("b".to_string()),
            bson::Bson::String("c".to_string()),
        ]
    );
}

#[tokio::test]
async fn find_one_update_return_expected_result() {
    let user = User::new(1);
    let filter = doc! {};
    let update = doc! { "$set": { "users.$[user].name": "siba" } };
    let array_filters = [doc! {"user.id": { "$gte": 1 }}];
    let fake = FakeMongo::start().await;
    fake.expect_find_and_update()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(filter.clone())
        .update(update.clone())
        .upsert(true)
        .array_filters(array_filters.clone())
        .sort(doc! {"id": 1})
        .projection(doc! {"_id": 0})
        .return_document(ReturnDocument::After)
        .respond_with(Some(user.clone()));
    let coll = fake.db(DB_NAME).await.collection::<User>(COLLECTION);
    let res = coll
        .find_one_and_update(filter, update)
        .sort(doc! {"id": 1})
        .projection(doc! {"_id": 0})
        .upsert(true)
        .array_filters(array_filters)
        .return_document(ReturnDocument::After)
        .await
        .unwrap();
    assert_eq!(res, Some(user));
}

#[tokio::test]
async fn find_one_replace_return_expected_result() {
    let user = User::new(1);
    let filter = doc! {};
    let fake = FakeMongo::start().await;
    fake.expect_find_and_replace()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(filter.clone())
        .sort(doc! {"id": 1})
        .projection(doc! {"_id": 0})
        .return_document(ReturnDocument::After)
        .respond_with(Some(user.clone()));
    let coll = fake.db(DB_NAME).await.collection::<User>(COLLECTION);
    let res = coll
        .find_one_and_replace(filter, user.clone())
        .upsert(true)
        .sort(doc! {"id": 1})
        .projection(doc! {"_id": 0})
        .return_document(ReturnDocument::After)
        .await
        .unwrap();
    assert_eq!(res, Some(user));
}

#[tokio::test]
async fn find_one_delete_return_expected_result() {
    let user = User::new(1);
    let filter = doc! {};
    let fake = FakeMongo::start().await;
    fake.expect_find_and_delete()
        .database(DB_NAME)
        .collection(COLLECTION)
        .filter(filter.clone())
        .sort(doc! {"id": 1})
        .projection(doc! {"_id": 0})
        .respond_with(Some(user.clone()));
    let coll = fake.db(DB_NAME).await.collection::<User>(COLLECTION);
    let res = coll
        .find_one_and_delete(filter)
        .sort(doc! {"id": 1})
        .projection(doc! {"_id": 0})
        .await
        .unwrap();
    assert_eq!(res, Some(user));
}

#[tokio::test]
async fn create_index_returns_expected_result() {
    let index_model = IndexModel::builder().keys(doc! {"id": 1}).build();
    let fake = FakeMongo::start().await;
    fake.expect_create_index();
    let coll = fake.db(DB_NAME).await.collection::<User>(COLLECTION);
    let res = coll.create_index(index_model).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn find_one_works_with_custom_validator() {
    let expected_id = 23;
    let fake = FakeMongo::start().await;
    fake.expect_find_one()
        .validate_with(|command| {
            let coll = command.get_str("find").unwrap_or_default();
            let db = command.get_str("$db").unwrap_or_default();
            if db != DB_NAME {
                anyhow::bail!("database name do not match");
            }
            if coll != COLLECTION {
                anyhow::bail!("collection name do not match");
            }
            Ok(())
        })
        .respond_with(&User::new(expected_id));
    let client = fake.client().await;
    let user = find_user_by_id(&client, 23).await.unwrap();
    assert_eq!(user.id, expected_id);
}
