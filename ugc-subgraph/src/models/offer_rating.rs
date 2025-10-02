use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct OfferRating {
    pub offer_id: Uuid,
    pub average_rating: Decimal,
    pub reviews_count: i32,
    pub rating_distribution: JsonValue,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingDistribution {
    pub rating_1: i32,
    pub rating_2: i32,
    pub rating_3: i32,
    pub rating_4: i32,
    pub rating_5: i32,
}

impl OfferRating {
    /// Convert the JSON rating distribution to a structured format
    pub fn get_rating_distribution(&self) -> Result<RatingDistribution, serde_json::Error> {
        let distribution_map: HashMap<String, i32> = serde_json::from_value(self.rating_distribution.clone())?;
        
        Ok(RatingDistribution {
            rating_1: distribution_map.get("1").copied().unwrap_or(0),
            rating_2: distribution_map.get("2").copied().unwrap_or(0),
            rating_3: distribution_map.get("3").copied().unwrap_or(0),
            rating_4: distribution_map.get("4").copied().unwrap_or(0),
            rating_5: distribution_map.get("5").copied().unwrap_or(0),
        })
    }

    /// Get the average rating as a float
    pub fn average_rating_f64(&self) -> f64 {
        self.average_rating.to_string().parse().unwrap_or(0.0)
    }

    /// Check if the rating data is fresh (updated within the last hour)
    pub fn is_fresh(&self) -> bool {
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);
        self.updated_at > one_hour_ago
    }

    /// Get the percentage of reviews for each rating
    pub fn get_rating_percentages(&self) -> Result<HashMap<i32, f64>, serde_json::Error> {
        let distribution = self.get_rating_distribution()?;
        let total = self.reviews_count as f64;
        
        if total == 0.0 {
            return Ok(HashMap::new());
        }

        let mut percentages = HashMap::new();
        percentages.insert(1, (distribution.rating_1 as f64 / total) * 100.0);
        percentages.insert(2, (distribution.rating_2 as f64 / total) * 100.0);
        percentages.insert(3, (distribution.rating_3 as f64 / total) * 100.0);
        percentages.insert(4, (distribution.rating_4 as f64 / total) * 100.0);
        percentages.insert(5, (distribution.rating_5 as f64 / total) * 100.0);

        Ok(percentages)
    }
}

impl Default for RatingDistribution {
    fn default() -> Self {
        Self {
            rating_1: 0,
            rating_2: 0,
            rating_3: 0,
            rating_4: 0,
            rating_5: 0,
        }
    }
}

impl RatingDistribution {
    pub fn total_reviews(&self) -> i32 {
        self.rating_1 + self.rating_2 + self.rating_3 + self.rating_4 + self.rating_5
    }

    pub fn to_json(&self) -> JsonValue {
        serde_json::json!({
            "1": self.rating_1,
            "2": self.rating_2,
            "3": self.rating_3,
            "4": self.rating_4,
            "5": self.rating_5,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_rating_distribution_total() {
        let distribution = RatingDistribution {
            rating_1: 1,
            rating_2: 2,
            rating_3: 3,
            rating_4: 4,
            rating_5: 5,
        };
        assert_eq!(distribution.total_reviews(), 15);
    }

    #[test]
    fn test_rating_distribution_to_json() {
        let distribution = RatingDistribution {
            rating_1: 1,
            rating_2: 2,
            rating_3: 3,
            rating_4: 4,
            rating_5: 5,
        };
        let json = distribution.to_json();
        assert_eq!(json["1"], 1);
        assert_eq!(json["5"], 5);
    }

    #[test]
    fn test_offer_rating_average_f64() {
        let offer_rating = OfferRating {
            offer_id: Uuid::new_v4(),
            average_rating: dec!(4.25),
            reviews_count: 10,
            rating_distribution: serde_json::json!({"1": 0, "2": 1, "3": 2, "4": 3, "5": 4}),
            updated_at: Utc::now(),
        };
        
        assert_eq!(offer_rating.average_rating_f64(), 4.25);
    }

    #[test]
    fn test_offer_rating_is_fresh() {
        let fresh_rating = OfferRating {
            offer_id: Uuid::new_v4(),
            average_rating: dec!(4.25),
            reviews_count: 10,
            rating_distribution: serde_json::json!({}),
            updated_at: Utc::now(),
        };
        assert!(fresh_rating.is_fresh());

        let old_rating = OfferRating {
            offer_id: Uuid::new_v4(),
            average_rating: dec!(4.25),
            reviews_count: 10,
            rating_distribution: serde_json::json!({}),
            updated_at: Utc::now() - chrono::Duration::hours(2),
        };
        assert!(!old_rating.is_fresh());
    }

    #[test]
    fn test_get_rating_distribution() {
        let offer_rating = OfferRating {
            offer_id: Uuid::new_v4(),
            average_rating: dec!(4.25),
            reviews_count: 10,
            rating_distribution: serde_json::json!({"1": 0, "2": 1, "3": 2, "4": 3, "5": 4}),
            updated_at: Utc::now(),
        };
        
        let distribution = offer_rating.get_rating_distribution().unwrap();
        assert_eq!(distribution.rating_1, 0);
        assert_eq!(distribution.rating_2, 1);
        assert_eq!(distribution.rating_3, 2);
        assert_eq!(distribution.rating_4, 3);
        assert_eq!(distribution.rating_5, 4);
    }

    #[test]
    fn test_get_rating_percentages() {
        let offer_rating = OfferRating {
            offer_id: Uuid::new_v4(),
            average_rating: dec!(4.0),
            reviews_count: 10,
            rating_distribution: serde_json::json!({"1": 0, "2": 0, "3": 0, "4": 5, "5": 5}),
            updated_at: Utc::now(),
        };
        
        let percentages = offer_rating.get_rating_percentages().unwrap();
        assert_eq!(percentages[&4], 50.0);
        assert_eq!(percentages[&5], 50.0);
        assert_eq!(percentages[&1], 0.0);
    }
}