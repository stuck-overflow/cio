use std::env;

use async_trait::async_trait;
use gsuite_api::GSuite;
use macros::db;
use okta::Okta;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::airtable::{AIRTABLE_BASE_ID_FINANCE, AIRTABLE_SOFTWARE_VENDORS_TABLE};
use crate::configs::Group;
use crate::core::UpdateAirtableRecord;
use crate::db::Database;
use crate::schema::software_vendors;
use crate::utils::{authenticate_github_jwt, get_gsuite_token, github_org, GSUITE_DOMAIN};

#[db {
    new_struct_name = "SoftwareVendor",
    airtable_base_id = "AIRTABLE_BASE_ID_FINANCE",
    airtable_table = "AIRTABLE_SOFTWARE_VENDORS_TABLE",
    match_on = {
        "name" = "String",
    },
}]
#[derive(Debug, Insertable, AsChangeset, PartialEq, Clone, JsonSchema, Deserialize, Serialize)]
#[table_name = "software_vendors"]
pub struct NewSoftwareVendor {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub website: String,
    #[serde(default)]
    pub has_okta_integration: bool,
    #[serde(default)]
    pub used_purely_for_api: bool,
    #[serde(default)]
    pub pay_as_you_go: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pay_as_you_go_pricing_description: String,
    #[serde(default)]
    pub software_licenses: bool,
    #[serde(default)]
    pub cost_per_user_per_month: f32,
    #[serde(default)]
    pub users: i32,
    #[serde(default)]
    pub flat_cost_per_month: f32,
    #[serde(default)]
    pub total_cost_per_month: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
}

/// Implement updating the Airtable record for a SoftwareVendor.
#[async_trait]
impl UpdateAirtableRecord<SoftwareVendor> for SoftwareVendor {
    #[instrument]
    #[inline]
    async fn update_airtable_record(&mut self, _record: SoftwareVendor) {}
}

/// Sync software vendors from Airtable.
#[instrument]
#[inline]
pub async fn refresh_software_vendors() {
    let gsuite_customer = env::var("GADMIN_ACCOUNT_ID").unwrap();
    let token = get_gsuite_token("").await;
    let gsuite = GSuite::new(&gsuite_customer, GSUITE_DOMAIN, token.clone());

    let db = Database::new();

    let github = authenticate_github_jwt();

    let okta = Okta::new_from_env();

    let slack = slack_chat_api::Slack::new_from_env();

    // Get all the records from Airtable.
    let results: Vec<airtable_api::Record<SoftwareVendor>> = SoftwareVendor::airtable().list_records(&SoftwareVendor::airtable_table(), "Grid view", vec![]).await.unwrap();
    for vendor_record in results {
        let mut vendor: NewSoftwareVendor = vendor_record.fields.into();

        if vendor.name == "GitHub" {
            // Update the number of GitHub users in our org.
            let org = github.org(github_org()).get().await.unwrap();
            vendor.users = org.plan.filled_seats;
        }

        if vendor.name == "Okta" {
            let users = okta.list_users().await.unwrap();
            vendor.users = users.len() as i32;
        }

        if vendor.name == "Google Workspace" {
            let users = gsuite.list_users().await.unwrap();
            vendor.users = users.len() as i32;
        }

        if vendor.name == "Slack" {
            let users = slack.billable_info().await.unwrap();
            let mut count = 0;
            for (_, user) in users {
                if user.billing_active {
                    count += 1;
                }
            }

            vendor.users = count;
        }

        // Airtable, Brex, Gusto, Expensify are all the same number of users as
        // in all@.
        if vendor.name == "Airtable" || vendor.name == "Brex" || vendor.name == "Gusto" || vendor.name == "Expensify" {
            let group = Group::get_from_db(&db, "all".to_string()).unwrap();
            let airtable_group = group.get_existing_airtable_record().await.unwrap();
            vendor.users = airtable_group.fields.members.len() as i32;
        }

        // Upsert the record in our database.
        let mut db_vendor = vendor.upsert_in_db(&db);

        if db_vendor.airtable_record_id.is_empty() {
            db_vendor.airtable_record_id = vendor_record.id;
        }
        db_vendor.update(&db).await;
    }
}

#[cfg(test)]
mod tests {
    use crate::finance::refresh_software_vendors;

    #[ignore]
    #[tokio::test(threaded_scheduler)]
    async fn test_software_vendors() {
        refresh_software_vendors().await;
    }
}
