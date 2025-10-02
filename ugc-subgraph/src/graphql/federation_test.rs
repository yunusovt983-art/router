#[cfg(test)]
mod tests {
    use crate::graphql::types::{OfferType, UserType};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_federation_schema_structure() {
        // Test that our types have the correct structure for federation
        let user_id = Uuid::new_v4();
        let offer_id = Uuid::new_v4();
        
        let user = UserType { id: user_id };
        let offer = OfferType { id: offer_id };
        
        assert_eq!(user.id, user_id);
        assert_eq!(offer.id, offer_id);
    }
}