//! Customer profile, preferences, account status and profile image (IMP-P3-010).
//!
//! Every operation takes the customer from the authenticated session; none
//! accepts a customer identifier from a request. Owner decisions recorded on
//! 2026-11-25: names are editable until identity verification starts,
//! optional email notifications default on and push off, marketing consent can
//! be granted or withdrawn, and profile images follow the contract below.
//! Account status is the lifecycle state of FRS §8.4. It is neither a KYC
//! decision nor financial authorization.
use std::future::Future;
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};
use uuid::Uuid;

pub const MAX_NAME_CHARS: usize = 100;

/// Approved upload limit: 5 MB (5,242,880 bytes).
pub const MAX_UPLOAD_BYTES: usize = 5 * 1024 * 1024;
/// Approved limit for each side of an uploaded image.
pub const MAX_INPUT_SIDE: u32 = 4096;
/// The stored image's longest side. Smaller images are never enlarged.
pub const MAX_OUTPUT_SIDE: u32 = 512;
pub const JPEG_QUALITY: u8 = 85;
/// Approved stored-size cap: 150 KB (153,600 bytes).
pub const MAX_STORED_BYTES: usize = 150 * 1024;
/// Images decoded at once by one API instance.
pub const MAX_CONCURRENT_IMAGE_JOBS: usize = 2;

/// Account lifecycle status (FRS §8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    Registered,
    ContactUnverified,
    ContactVerified,
    KycRequired,
    KycPending,
    KycApproved,
    KycRejected,
    Restricted,
    Suspended,
    Closed,
}

impl AccountStatus {
    /// Parses the stored value. An unknown value is a data-integrity failure.
    pub fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "REGISTERED" => Self::Registered,
            "CONTACT_UNVERIFIED" => Self::ContactUnverified,
            "CONTACT_VERIFIED" => Self::ContactVerified,
            "KYC_REQUIRED" => Self::KycRequired,
            "KYC_PENDING" => Self::KycPending,
            "KYC_APPROVED" => Self::KycApproved,
            "KYC_REJECTED" => Self::KycRejected,
            "RESTRICTED" => Self::Restricted,
            "SUSPENDED" => Self::Suspended,
            "CLOSED" => Self::Closed,
            _ => return None,
        })
    }

    /// The public value: the FRS name in lowercase, as for KYC statuses (API §17.7).
    pub const fn public_name(self) -> &'static str {
        match self {
            Self::Registered => "registered",
            Self::ContactUnverified => "contact_unverified",
            Self::ContactVerified => "contact_verified",
            Self::KycRequired => "kyc_required",
            Self::KycPending => "kyc_pending",
            Self::KycApproved => "kyc_approved",
            Self::KycRejected => "kyc_rejected",
            Self::Restricted => "restricted",
            Self::Suspended => "suspended",
            Self::Closed => "closed",
        }
    }
}

/// Returns the trimmed name when it is 1–100 characters and contains no
/// control, invisible formatting, line/paragraph separator or non-space
/// whitespace character.
pub fn profile_name(input: &str) -> Option<&str> {
    let name = input.trim();
    let visible = name.chars().all(|c| {
        c == ' '
            || !(c.is_whitespace()
                || matches!(
                    c.general_category(),
                    GeneralCategory::Control
                        | GeneralCategory::Format
                        | GeneralCategory::LineSeparator
                        | GeneralCategory::ParagraphSeparator
                        | GeneralCategory::Surrogate
                ))
    });
    (visible && (1..=MAX_NAME_CHARS).contains(&name.chars().count())).then_some(name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageMetadata {
    /// Changes on every replacement; the API uses it as the ETag.
    pub version: Uuid,
    pub width: u32,
    pub height: u32,
    pub updated_at: String,
}

/// The customer's own profile. Timestamps are ISO 8601 UTC strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerProfile {
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub email_verified: bool,
    pub phone: Option<String>,
    pub phone_verified: bool,
    pub country_code: String,
    pub account_status: AccountStatus,
    pub names_editable: bool,
    pub image: Option<ImageMetadata>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preferences {
    pub phone_discoverable: bool,
    pub email_notifications: bool,
    pub push_notifications: bool,
    pub marketing_consent: bool,
}

/// `None` leaves a preference unchanged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreferenceChange {
    pub phone_discoverable: Option<bool>,
    pub email_notifications: Option<bool>,
    pub push_notifications: Option<bool>,
    pub marketing_consent: Option<bool>,
}

impl PreferenceChange {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileError {
    /// Storage was unreachable or failed; nothing was changed.
    Unavailable,
    /// Storage refused a value the API accepted.
    Rejected,
    /// Stored data violates the profile contract.
    Corrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Updated,
    /// Names are locked once identity verification has started.
    NamesLocked,
    /// Phone discovery requires a verified phone (API §16.5).
    PhoneUnverified,
    /// The account can no longer hold a customer session.
    Ineligible,
}

/// Persistence confines every operation to the customer it is given and
/// records each change with its audit event in the same transaction. `None`
/// fields leave values unchanged; at least one must be present.
pub trait ProfileStore: Send + Sync {
    fn profile(
        &self,
        customer: Uuid,
    ) -> impl Future<Output = Result<Option<CustomerProfile>, ProfileError>> + Send;
    fn rename(
        &self,
        customer: Uuid,
        first_name: Option<&str>,
        last_name: Option<&str>,
    ) -> impl Future<Output = Result<Outcome, ProfileError>> + Send;
    fn preferences(
        &self,
        customer: Uuid,
    ) -> impl Future<Output = Result<Option<(Preferences, Option<String>)>, ProfileError>> + Send;
    fn update_preferences(
        &self,
        customer: Uuid,
        change: PreferenceChange,
    ) -> impl Future<Output = Result<Outcome, ProfileError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameError {
    /// The listed fields failed validation.
    Invalid {
        first_name: bool,
        last_name: bool,
    },
    Store(Outcome),
    Failed(ProfileError),
}

/// Validates and applies a name change, then returns the updated profile.
pub async fn rename(
    store: &impl ProfileStore,
    customer: Uuid,
    first_name: Option<&str>,
    last_name: Option<&str>,
) -> Result<CustomerProfile, RenameError> {
    let first = first_name.map(profile_name);
    let last = last_name.map(profile_name);
    let (first_bad, last_bad) = (first == Some(None), last == Some(None));
    if first_bad || last_bad || (first.is_none() && last.is_none()) {
        return Err(RenameError::Invalid {
            first_name: first_bad,
            last_name: last_bad,
        });
    }
    match store
        .rename(customer, first.flatten(), last.flatten())
        .await
        .map_err(RenameError::Failed)?
    {
        Outcome::Updated => store
            .profile(customer)
            .await
            .map_err(RenameError::Failed)?
            .ok_or(RenameError::Store(Outcome::Ineligible)),
        other => Err(RenameError::Store(other)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadFormat {
    Jpeg,
    Png,
    WebP,
}

impl UploadFormat {
    /// Accepts exactly `image/jpeg`, `image/png` or `image/webp`, ignoring case
    /// and parameters.
    pub fn from_content_type(value: &str) -> Option<Self> {
        let media = value.split(';').next()?.trim();
        [
            ("image/jpeg", Self::Jpeg),
            ("image/png", Self::Png),
            ("image/webp", Self::WebP),
        ]
        .into_iter()
        .find_map(|(name, format)| media.eq_ignore_ascii_case(name).then_some(format))
    }
}

/// A re-encoded JPEG that satisfies the stored-image contract.
#[derive(Clone, PartialEq, Eq)]
pub struct NormalizedImage {
    jpeg: Vec<u8>,
    width: u32,
    height: u32,
}

impl NormalizedImage {
    pub fn new(jpeg: Vec<u8>, width: u32, height: u32) -> Option<Self> {
        let valid = (1..=MAX_OUTPUT_SIDE).contains(&width)
            && (1..=MAX_OUTPUT_SIDE).contains(&height)
            && (4..=MAX_STORED_BYTES).contains(&jpeg.len())
            && jpeg.starts_with(&[0xff, 0xd8, 0xff])
            && jpeg.ends_with(&[0xff, 0xd9]);
        valid.then_some(Self {
            jpeg,
            width,
            height,
        })
    }
    pub fn jpeg(&self) -> &[u8] {
        &self.jpeg
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
}

impl std::fmt::Debug for NormalizedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NormalizedImage")
            .field("bytes", &self.jpeg.len())
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageRejection {
    /// The bytes are not the declared format.
    TypeMismatch,
    /// A side exceeds the limit, or decoding would exceed the memory budget.
    TooManyPixels,
    /// Truncated, malformed or otherwise undecodable.
    Undecodable,
    /// Even the lowest permitted quality exceeds the stored-size cap.
    TooComplex,
}

/// Decodes untrusted bytes within the approved limits and re-encodes them.
/// CPU-bound: callers run it on a blocking thread with bounded concurrency.
pub trait ImageNormalizer: Send + Sync {
    fn normalize(
        &self,
        format: UploadFormat,
        bytes: &[u8],
    ) -> Result<NormalizedImage, ImageRejection>;
}

/// The current image, without its bytes when the caller already holds `version`.
pub struct StoredImage {
    pub version: Uuid,
    pub jpeg: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageRemoval {
    Removed,
    Absent,
    Ineligible,
}

/// Replacement and removal are atomic with their audit events. A failed write
/// leaves the previous image in place.
pub trait ProfileImageStore: Send + Sync {
    /// `None` when the account can no longer hold a customer session.
    fn store_image(
        &self,
        customer: Uuid,
        image: &NormalizedImage,
    ) -> impl Future<Output = Result<Option<ImageMetadata>, ProfileError>> + Send;
    fn image(
        &self,
        customer: Uuid,
        known_version: Option<Uuid>,
    ) -> impl Future<Output = Result<Option<StoredImage>, ProfileError>> + Send;
    fn remove_image(
        &self,
        customer: Uuid,
    ) -> impl Future<Output = Result<ImageRemoval, ProfileError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_bounded_and_visible() {
        for (input, expected) in [
            ("Ada", "Ada"),
            ("  Ada  ", "Ada"),
            ("Mary Jane", "Mary Jane"),
            ("O'Neil-Okafor", "O'Neil-Okafor"),
            ("Chiamaka \u{c9}ze", "Chiamaka \u{c9}ze"),
            ("\u{674e}", "\u{674e}"),
        ] {
            assert_eq!(profile_name(input), Some(expected), "{input:?}");
        }
        assert_eq!(profile_name(&"a".repeat(100)).map(str::len), Some(100));
        for input in [
            "",
            "   ",
            "A\u{200b}da",
            "\u{202e}Ada",
            "Ada\u{a0}O",
            "Ada\tO",
            "A\u{ad}da",
            "\u{feff}Ada",
            "Ada\u{2028}O",
            "Ada\u{3000}O",
            "A\u{0}da",
        ] {
            assert_eq!(profile_name(input), None, "{input:?}");
        }
        assert_eq!(profile_name(&"a".repeat(101)), None);
        assert_eq!(profile_name(&"\u{e9}".repeat(101)), None);
    }

    #[test]
    fn every_stored_status_has_one_lowercase_public_value() {
        let all = [
            "REGISTERED",
            "CONTACT_UNVERIFIED",
            "CONTACT_VERIFIED",
            "KYC_REQUIRED",
            "KYC_PENDING",
            "KYC_APPROVED",
            "KYC_REJECTED",
            "RESTRICTED",
            "SUSPENDED",
            "CLOSED",
        ];
        for stored in all {
            let status = AccountStatus::parse(stored).unwrap();
            assert_eq!(status.public_name(), stored.to_ascii_lowercase());
        }
        for unknown in ["", "contact_verified", "ACTIVE", "KYC_APPROVED "] {
            assert_eq!(AccountStatus::parse(unknown), None);
        }
    }

    #[test]
    fn upload_types_are_exact() {
        assert_eq!(
            UploadFormat::from_content_type("image/jpeg"),
            Some(UploadFormat::Jpeg)
        );
        assert_eq!(
            UploadFormat::from_content_type("IMAGE/PNG; charset=binary"),
            Some(UploadFormat::Png)
        );
        assert_eq!(
            UploadFormat::from_content_type("image/webp"),
            Some(UploadFormat::WebP)
        );
        for other in [
            "image/jpg",
            "image/gif",
            "image/svg+xml",
            "image/heic",
            "application/octet-stream",
            "",
            "image/jpeg2",
        ] {
            assert_eq!(UploadFormat::from_content_type(other), None, "{other}");
        }
    }

    #[test]
    fn normalized_images_must_be_bounded_jpegs() {
        let jpeg = |len: usize| {
            let mut bytes = vec![0xff, 0xd8, 0xff];
            bytes.resize(len - 2, 0);
            bytes.extend([0xff, 0xd9]);
            bytes
        };
        assert!(NormalizedImage::new(jpeg(100), 512, 1).is_some());
        assert!(NormalizedImage::new(jpeg(MAX_STORED_BYTES), 1, 512).is_some());
        assert!(NormalizedImage::new(jpeg(MAX_STORED_BYTES + 1), 10, 10).is_none());
        assert!(NormalizedImage::new(jpeg(100), 513, 10).is_none());
        assert!(NormalizedImage::new(jpeg(100), 0, 10).is_none());
        let mut png = jpeg(100);
        png[0] = 0x89;
        assert!(NormalizedImage::new(png, 10, 10).is_none());
        let mut truncated = jpeg(100);
        truncated.pop();
        assert!(NormalizedImage::new(truncated, 10, 10).is_none());
    }
}
