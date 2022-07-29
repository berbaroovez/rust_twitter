use crate::db::user::WhereParam;
use crate::db::*;
use chrono::Local;
use std::env;
use std::error::Error;
use twitter_v2::authorization::BearerToken;
use twitter_v2::query::UserField;
use twitter_v2::TwitterApi;
pub mod db;
#[derive(Debug)]
enum ChangeType {
    Unfollowed,
    Followed,
}

#[tokio::main]
pub async fn main() {
    let client = new_client().await.unwrap();
    let follower_list_response = get_twitter_followers().await;

    let users: Vec<user::Data> = client.user().find_many(vec![]).exec().await.unwrap();

    // let list_of_people_that_unfollowed: Vec<&user::Data>;

    match follower_list_response {
        Ok(list) => {
            let list_of_new_followers = are_you_a_new_follower(&users, &list);
            let list_of_people_that_unfollowed = are_you_following_still_check(&users, &list);
            // let yoo = list_of_new_followers;

            insert_into_change_table(list_of_new_followers, ChangeType::Followed).await;
            insert_into_change_table(list_of_people_that_unfollowed, ChangeType::Unfollowed).await;
            // println!("New followers, {:?}", list_of_new_followers)
        }
        Err(e) => panic!("Something went wrong"),
    };

    // insert_into_change_table(list_of_people_that_unfollowed, ChangeType::Unfollowed).await;
}

fn are_you_a_new_follower<'a>(
    database_user_list: &[user::Data],
    twitter_user_list: &'a [user::Data],
) -> Vec<&'a user::Data> {
    println!("Checking for new followers");
    let database_user_hashset = database_user_list
        .iter()
        .map(|user| (&user.user_id, &user.status))
        .collect::<std::collections::HashMap<_, _>>();
    let mut new_follower_list: Vec<&user::Data> = vec![];

    println!("{:?}", database_user_hashset);

    for user in twitter_user_list {
        let mut new_follower = false;

        if !database_user_hashset.contains_key(&user.user_id) {
            new_follower = true;
        } else {
            println!("This user is : {}", database_user_hashset[&user.user_id]);
            if database_user_hashset[&user.user_id].eq("shady") {
                new_follower = true;
            }
        }

        if new_follower == true {
            new_follower_list.push(user)
        }
    }

    new_follower_list
}
//returns a vector of all the users that are in the database but are not currently following me on twitter
//we also check to see if there status is "following" this means they are a fresh unfollow
//if it was "shady" they already previously unfollowed
fn are_you_following_still_check<'a>(
    database_user_list: &'a [user::Data],
    twitter_user_list: &[user::Data],
) -> Vec<&'a user::Data> {
    println!("Inside shady check");
    let twitter_ids = twitter_user_list
        .iter()
        .map(|user| &user.user_id)
        .collect::<std::collections::HashSet<_>>();

    //this will return any user that currently in the database is not found on the twitter
    database_user_list
        .iter()
        .filter(|user| {
            !twitter_ids.contains(&user.user_id) && matches!(user.status.as_str(), "following")
        })
        .collect()
}

async fn insert_into_change_table(user_list: Vec<&user::Data>, type_of_change: ChangeType) {
    println!("Inserting in change table");
    println!("Type of change is {:?}", type_of_change);
    println!("list data:, {:?}", &user_list);

    let client = new_client().await.unwrap();
    for user in user_list {
        // client
        //     .user()
        //     .find_unique(user::user_id::equals(user.user_id.clone()))
        //     .update(vec![user::status::set(match type_of_change {
        //         ChangeType::Followed => "following".to_string(),
        //         ChangeType::Unfollowed => "shady".to_string(),
        //     })])
        //     .exec()
        //     .await;

        let upsert_into_users = client
            .user()
            .upsert(
                user::user_id::equals(user.user_id.clone()),
                (
                    user::user_id::set(user.user_id.clone()),
                    user::username::set(user.username.clone()),
                    user::name::set(user.name.clone()),
                    user::verified::set(user.verified.clone()),
                    user::status::set(user.status.clone()),
                    vec![],
                ),
                vec![user::status::set(match type_of_change {
                    ChangeType::Followed => "following".to_string(),
                    ChangeType::Unfollowed => "shady".to_string(),
                })],
            )
            .exec()
            .await;

        client
            .change()
            .create(
                change::change_type::set(match type_of_change {
                    ChangeType::Followed => "followed".to_string(),
                    ChangeType::Unfollowed => "shady".to_string(),
                }),
                change::user::link(user::user_id::equals(user.user_id.clone())),
                change::date::set(Local::now().to_string()),
                vec![],
            )
            .exec()
            .await
            .unwrap();
    }
}

// #[tokio::main]
async fn get_twitter_followers() -> Result<Vec<user::Data>, Box<dyn Error>> {
    let token = env::var("TWITTER_TRACKER_BEARER_TOKEN");

    // for (key, value) in env::vars() {
    //     println!("{key}: {value}");
    // }

    let mut user_vector: Vec<user::Data> = Vec::new();
    match token {
        Ok(bearer_token) => {
            let auth = BearerToken::new(bearer_token);

            let follower_vector = TwitterApi::new(auth)
                .get_user_followers(1036021604898226178)
                .user_fields([UserField::Verified])
                .max_results(1000)
                .send()
                .await?
                .into_data();

            match follower_vector {
                Some(followers) => {
                    for follower in &followers[..] {
                        let user_data = user::Data {
                            user_id: follower.id.to_string(),
                            name: follower.name.to_string(),
                            username: follower.username.to_string(),
                            verified: match follower.verified {
                                Some(true) => String::from("true"),
                                Some(false) => String::from("false"),
                                None => String::from("false"),
                            },
                            status: String::from("following"),
                            changes: None,
                        };
                        user_vector.push(user_data)
                    }
                } //end of some match
                None => println!("No followers"),
            } //end of match
        }
        Err(e) => panic!("Oh no {}", e),
    }

    Ok(user_vector)
}
