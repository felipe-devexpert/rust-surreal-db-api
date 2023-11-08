use actix_web::web::Data;
use serde::{Deserialize, Serialize};
use surrealdb::{
    engine::remote::ws::Client,
    sql::{Thing, Value},
    Surreal,
};

use crate::{app_error::AppError, data_map, utils::password};

const TABLE_NAME: &str = "user";

pub enum UserExists {
    Username,
    EmailId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: Option<Thing>,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub email_id: String,
    pub password: String,
    pub age: u8,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone)]
pub enum UserFindableCol {
    #[allow(dead_code)]
    Username,

    #[allow(dead_code)]
    EmailId,

    #[allow(dead_code)]
    FirstName,

    #[allow(dead_code)]
    LastName,
}
impl Into<String> for UserFindableCol {
    fn into(self) -> String {
        match self {
            Self::Username => "username".into(),
            Self::EmailId => "email_id".into(),
            Self::FirstName => "first_name".into(),
            Self::LastName => "last_name".into(),
        }
    }
}

impl From<User> for Value {
    fn from(user: User) -> Self {
        let mut user_map = data_map![
            "username" => user.username.into(),
            "first_name" => user.first_name.into(),
            "last_name" => user.last_name.into(),
            "email_id" => user.email_id.into(),
            "password" => user.password.into(),
            "age" => user.age.into(),
            "avatar" => user.avatar.into(),
        ];

        // Checks if this is a new user or not
        if let Some(id) = user.id {
            user_map.insert("id".into(), id.into());
        }

        user_map.into()
    }
}
impl User {
    pub async fn get_all(db: &Data<Surreal<Client>>) -> Result<Vec<User>, AppError> {
        let q = "SELECT * from type::table($tb);";

        let mut response = db.query(q).bind(("tb", TABLE_NAME)).await?;

        let users = response.take::<Vec<User>>(0)?;

        Ok(users)
    }
    /// Creates a new user only if the email and username does not exist in the database
    /// Hashing of the password is done in this function
    pub async fn create(&mut self, db: &Data<Surreal<Client>>) -> Result<Option<User>, AppError> {
        let existing = self.exists(&db).await?;

        if let Some(existing) = existing {
            match existing {
                UserExists::Username => {
                    return Err(AppError::BadRequest(format!(
                        "The user with username '{}' already exists...",
                        self.username,
                    )))
                }
                UserExists::EmailId => {
                    return Err(AppError::BadRequest(format!(
                        "The user with email_id '{}' already exists!",
                        self.email_id,
                    )))
                }
            }
        }

        self.password = password::hash_password(&self.password)?;
        let q = "CREATE type::table($table) CONTENT $data RETURN *";

        let vars = data_map!["table" => TABLE_NAME.into(), "data" => self.clone().into()];

        let mut db_response = db.query(q).bind(vars).await?;

        let user: Option<User> = db_response.take(0)?;

        Ok(user)
    }

    pub async fn verify_password(&self, password: &str) -> Result<bool, AppError> {
        password::verify(password, &self.password)
    }

    pub async fn find_one(
        db: &Data<Surreal<Client>>,
        search_term: UserFindableCol,
        value: impl Into<Value>,
    ) -> Result<Option<User>, AppError> {
        let search_term: String = search_term.into();
        let find_q = format!(
            "SELECT * FROM type::table($table) where {} = $value;",
            search_term
        );

        let vars = data_map![
            "table" => TABLE_NAME.into(),
            "value" => value.into()
        ];

        let mut db_res = db.query(find_q).bind(vars).await?;

        let db_res: Vec<User> = db_res.take(0)?;

        if db_res.len() == 0 {
            return Ok(None);
        }
        let user = db_res[0].clone();

        Ok(Some(user))
    }

    pub async fn exists(&self, db: &Data<Surreal<Client>>) -> Result<Option<UserExists>, AppError> {
        let find_q = "SELECT username, email_id FROM type::table($table) where username = $username or email_id = $email";

        let vars = data_map![
            "table" => TABLE_NAME.into(),
            "username" => self.username.clone().into(),
            "email" => self.email_id.clone().into(),
        ];

        let mut db_res = db.query(find_q).bind(vars).await?;
        let existing_username: Option<String> = db_res.take("username")?;
        let existing_email: Option<String> = db_res.take("email_id")?;

        if let Some(username) = existing_username {
            if username == self.username {
                return Ok(Some(UserExists::Username));
            }
        }

        if let Some(email) = existing_email {
            if email == self.email_id {
                return Ok(Some(UserExists::EmailId));
            }
        }

        Ok(None)
    }
}