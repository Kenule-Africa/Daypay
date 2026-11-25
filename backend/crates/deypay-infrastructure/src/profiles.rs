//! PostgreSQL customer profile, preference and profile-image store (IMP-P3-010).
//!
//! The runtime calls only the fixed `customers.*` operations granted by
//! `profile-grants.psql`. Each operation is confined to the customer UUID it
//! receives; the database cannot tell which customer the runtime serves, so
//! that UUID must come from the authenticated session.
use deypay_application::profile::{
    AccountStatus, CustomerProfile, ImageMetadata, ImageRemoval, NormalizedImage, Outcome,
    PreferenceChange, Preferences, ProfileError, ProfileImageStore, ProfileStore, StoredImage,
};
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

pub struct PgProfiles(pub PgPool);

fn failed(error: sqlx::Error) -> ProfileError {
    match error.as_database_error().and_then(|e| e.code()).as_deref() {
        // Invalid parameter or constraint: the database refused a value.
        Some("22023" | "23514") => ProfileError::Rejected,
        _ => ProfileError::Unavailable,
    }
}

fn dimension(value: i32) -> Result<u32, ProfileError> {
    u32::try_from(value).map_err(|_| ProfileError::Corrupt)
}

fn outcome(value: &str) -> Result<Outcome, ProfileError> {
    match value {
        "updated" => Ok(Outcome::Updated),
        "locked" => Ok(Outcome::NamesLocked),
        "phone_unverified" => Ok(Outcome::PhoneUnverified),
        "ineligible" => Ok(Outcome::Ineligible),
        _ => Err(ProfileError::Corrupt),
    }
}

fn profile(row: PgRow) -> Result<CustomerProfile, ProfileError> {
    let get = |e: sqlx::Error| {
        tracing::warn!(error = %e, "Profile row has an unexpected shape");
        ProfileError::Corrupt
    };
    let status: String = row.try_get("account_status").map_err(get)?;
    let image = match row
        .try_get::<Option<Uuid>, _>("image_version")
        .map_err(get)?
    {
        Some(version) => Some(ImageMetadata {
            version,
            width: dimension(row.try_get("image_width").map_err(get)?)?,
            height: dimension(row.try_get("image_height").map_err(get)?)?,
            updated_at: row.try_get("image_updated_at").map_err(get)?,
        }),
        None => None,
    };
    Ok(CustomerProfile {
        username: row.try_get("username").map_err(get)?,
        first_name: row.try_get("first_name").map_err(get)?,
        last_name: row.try_get("last_name").map_err(get)?,
        email: row.try_get("email").map_err(get)?,
        email_verified: row.try_get("email_verified").map_err(get)?,
        phone: row.try_get("phone").map_err(get)?,
        phone_verified: row.try_get("phone_verified").map_err(get)?,
        country_code: row.try_get("country_code").map_err(get)?,
        account_status: AccountStatus::parse(&status).ok_or(ProfileError::Corrupt)?,
        names_editable: row.try_get("names_editable").map_err(get)?,
        image,
        created_at: row.try_get("created_at").map_err(get)?,
        updated_at: row.try_get("updated_at").map_err(get)?,
    })
}

impl ProfileStore for PgProfiles {
    async fn profile(&self, customer: Uuid) -> Result<Option<CustomerProfile>, ProfileError> {
        sqlx::query("SELECT * FROM customers.customer_profile($1)")
            .bind(customer)
            .fetch_optional(&self.0)
            .await
            .map_err(failed)?
            .map(profile)
            .transpose()
    }

    async fn rename(
        &self,
        customer: Uuid,
        first_name: Option<&str>,
        last_name: Option<&str>,
    ) -> Result<Outcome, ProfileError> {
        let result: String = sqlx::query_scalar("SELECT customers.rename_customer($1, $2, $3)")
            .bind(customer)
            .bind(first_name)
            .bind(last_name)
            .fetch_one(&self.0)
            .await
            .map_err(failed)?;
        outcome(&result)
    }

    async fn preferences(
        &self,
        customer: Uuid,
    ) -> Result<Option<(Preferences, Option<String>)>, ProfileError> {
        let row: Option<(bool, bool, bool, bool, Option<String>)> =
            sqlx::query_as("SELECT * FROM customers.customer_preferences($1)")
                .bind(customer)
                .fetch_optional(&self.0)
                .await
                .map_err(failed)?;
        Ok(row.map(|(phone, email, push, marketing, updated_at)| {
            (
                Preferences {
                    phone_discoverable: phone,
                    email_notifications: email,
                    push_notifications: push,
                    marketing_consent: marketing,
                },
                updated_at,
            )
        }))
    }

    async fn update_preferences(
        &self,
        customer: Uuid,
        change: PreferenceChange,
    ) -> Result<Outcome, ProfileError> {
        let result: String =
            sqlx::query_scalar("SELECT customers.update_customer_preferences($1, $2, $3, $4, $5)")
                .bind(customer)
                .bind(change.phone_discoverable)
                .bind(change.email_notifications)
                .bind(change.push_notifications)
                .bind(change.marketing_consent)
                .fetch_one(&self.0)
                .await
                .map_err(failed)?;
        outcome(&result)
    }
}

impl ProfileImageStore for PgProfiles {
    async fn store_image(
        &self,
        customer: Uuid,
        image: &NormalizedImage,
    ) -> Result<Option<ImageMetadata>, ProfileError> {
        let row: Option<(Uuid, i32, i32, String)> =
            sqlx::query_as("SELECT * FROM customers.store_profile_image($1, $2, $3, $4)")
                .bind(customer)
                .bind(image.jpeg())
                .bind(i32::try_from(image.width()).map_err(|_| ProfileError::Rejected)?)
                .bind(i32::try_from(image.height()).map_err(|_| ProfileError::Rejected)?)
                .fetch_optional(&self.0)
                .await
                .map_err(failed)?;
        row.map(|(version, width, height, updated_at)| {
            Ok(ImageMetadata {
                version,
                width: dimension(width)?,
                height: dimension(height)?,
                updated_at,
            })
        })
        .transpose()
    }

    async fn image(
        &self,
        customer: Uuid,
        known_version: Option<Uuid>,
    ) -> Result<Option<StoredImage>, ProfileError> {
        let row: Option<(Uuid, Option<Vec<u8>>)> =
            sqlx::query_as("SELECT * FROM customers.profile_image($1, $2)")
                .bind(customer)
                .bind(known_version)
                .fetch_optional(&self.0)
                .await
                .map_err(failed)?;
        Ok(row.map(|(version, jpeg)| StoredImage { version, jpeg }))
    }

    async fn remove_image(&self, customer: Uuid) -> Result<ImageRemoval, ProfileError> {
        let result: String = sqlx::query_scalar("SELECT customers.remove_profile_image($1)")
            .bind(customer)
            .fetch_one(&self.0)
            .await
            .map_err(failed)?;
        match result.as_str() {
            "removed" => Ok(ImageRemoval::Removed),
            "absent" => Ok(ImageRemoval::Absent),
            "ineligible" => Ok(ImageRemoval::Ineligible),
            _ => Err(ProfileError::Corrupt),
        }
    }
}
