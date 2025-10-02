use async_graphql::Enum;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Review {
    pub id: Uuid,
    pub offer_id: Uuid,
    pub author_id: Uuid,
    pub rating: i32,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_moderated: bool,
    pub moderation_status: ModerationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, Enum, Copy, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum ModerationStatus {
    Pending,
    Approved,
    Rejected,
    Flagged,
}

impl Default for ModerationStatus {
    fn default() -> Self {
        ModerationStatus::Pending
    }
}

impl std::fmt::Display for ModerationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModerationStatus::Pending => write!(f, "pending"),
            ModerationStatus::Approved => write!(f, "approved"),
            ModerationStatus::Rejected => write!(f, "rejected"),
            ModerationStatus::Flagged => write!(f, "flagged"),
        }
    }
}

impl std::str::FromStr for ModerationStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(ModerationStatus::Pending),
            "approved" => Ok(ModerationStatus::Approved),
            "rejected" => Ok(ModerationStatus::Rejected),
            "flagged" => Ok(ModerationStatus::Flagged),
            _ => Err(format!("Invalid moderation status: {}", s)),
        }
    }
}

// Input types for GraphQL mutations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReviewInput {
    pub offer_id: Uuid,
    pub rating: i32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReviewInput {
    pub rating: Option<i32>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewsFilter {
    pub offer_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub min_rating: Option<i32>,
    pub max_rating: Option<i32>,
    pub moderated_only: Option<bool>,
    pub moderation_status: Option<ModerationStatus>,
}

impl CreateReviewInput {
    pub fn validate(&self) -> Result<(), String> {
        if self.rating < 1 || self.rating > 5 {
            return Err("Rating must be between 1 and 5".to_string());
        }
        
        if self.text.trim().is_empty() {
            return Err("Review text cannot be empty".to_string());
        }
        
        if self.text.len() > 5000 {
            return Err("Review text cannot exceed 5000 characters".to_string());
        }
        
        Ok(())
    }
}

impl UpdateReviewInput {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(rating) = self.rating {
            if rating < 1 || rating > 5 {
                return Err("Rating must be between 1 and 5".to_string());
            }
        }
        
        if let Some(text) = &self.text {
            if text.trim().is_empty() {
                return Err("Review text cannot be empty".to_string());
            }
            
            if text.len() > 5000 {
                return Err("Review text cannot exceed 5000 characters".to_string());
            }
        }
        
        Ok(())
    }
}



impl Default for ReviewsFilter {
    fn default() -> Self {
        Self {
            offer_id: None,
            author_id: None,
            min_rating: None,
            max_rating: None,
            moderated_only: None,
            moderation_status: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moderation_status_display() {
        assert_eq!(ModerationStatus::Pending.to_string(), "pending");
        assert_eq!(ModerationStatus::Approved.to_string(), "approved");
        assert_eq!(ModerationStatus::Rejected.to_string(), "rejected");
        assert_eq!(ModerationStatus::Flagged.to_string(), "flagged");
    }

    #[test]
    fn test_moderation_status_from_str() {
        assert!(matches!("pending".parse::<ModerationStatus>(), Ok(ModerationStatus::Pending)));
        assert!(matches!("approved".parse::<ModerationStatus>(), Ok(ModerationStatus::Approved)));
        assert!(matches!("rejected".parse::<ModerationStatus>(), Ok(ModerationStatus::Rejected)));
        assert!(matches!("flagged".parse::<ModerationStatus>(), Ok(ModerationStatus::Flagged)));
        assert!("invalid".parse::<ModerationStatus>().is_err());
    }

    #[test]
    fn test_create_review_input_validation() {
        let valid_input = CreateReviewInput {
            offer_id: Uuid::new_v4(),
            rating: 5,
            text: "Great car!".to_string(),
        };
        assert!(valid_input.validate().is_ok());

        let invalid_rating = CreateReviewInput {
            offer_id: Uuid::new_v4(),
            rating: 6,
            text: "Great car!".to_string(),
        };
        assert!(invalid_rating.validate().is_err());

        let empty_text = CreateReviewInput {
            offer_id: Uuid::new_v4(),
            rating: 5,
            text: "".to_string(),
        };
        assert!(empty_text.validate().is_err());

        let long_text = CreateReviewInput {
            offer_id: Uuid::new_v4(),
            rating: 5,
            text: "a".repeat(5001),
        };
        assert!(long_text.validate().is_err());
    }

    #[test]
    fn test_update_review_input_validation() {
        let valid_input = UpdateReviewInput {
            rating: Some(4),
            text: Some("Updated review".to_string()),
        };
        assert!(valid_input.validate().is_ok());

        let invalid_rating = UpdateReviewInput {
            rating: Some(0),
            text: None,
        };
        assert!(invalid_rating.validate().is_err());

        let empty_text = UpdateReviewInput {
            rating: None,
            text: Some("".to_string()),
        };
        assert!(empty_text.validate().is_err());
    }
}