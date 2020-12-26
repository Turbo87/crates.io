use crate::controllers::prelude::*;
use crate::models::Team;
use crate::util::{json_response, EndpointResult};
use conduit::RequestExt;

pub fn pager_duty(req: &mut dyn RequestExt) -> EndpointResult {
    let authenticated_user = req.authenticate()?;
    let user = authenticated_user.user();

    let org_id = dotenv::var("ADMIN_TEAM_ORG_ID")?.parse()?;
    let team_id = dotenv::var("ADMIN_TEAM_ID")?.parse()?;

    let result = Team::with_gh_id_contains_user(req.app(), org_id, team_id, &user);
    println!("is admin {:?}", result);

    let is_admin = result?;
    if !is_admin {
        // TODO throw error
    }

    println!("is admin {:?}", is_admin);

    let payload = json!({});
    Ok(json_response(&payload))
}
