-- Create offer_ratings table for aggregated ratings
CREATE TABLE offer_ratings (
    offer_id UUID PRIMARY KEY,
    average_rating DECIMAL(3,2) NOT NULL CHECK (average_rating >= 1.00 AND average_rating <= 5.00),
    reviews_count INTEGER NOT NULL DEFAULT 0 CHECK (reviews_count >= 0),
    rating_distribution JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create index for updated_at for maintenance queries
CREATE INDEX idx_offer_ratings_updated_at ON offer_ratings (updated_at);

-- Create function to update offer ratings when reviews change
CREATE OR REPLACE FUNCTION update_offer_rating()
RETURNS TRIGGER AS $$
BEGIN
    -- Update or insert offer rating
    INSERT INTO offer_ratings (offer_id, average_rating, reviews_count, rating_distribution, updated_at)
    SELECT 
        r.offer_id,
        ROUND(AVG(r.rating::DECIMAL), 2) as average_rating,
        COUNT(*) as reviews_count,
        jsonb_object_agg(r.rating::TEXT, rating_counts.count) as rating_distribution,
        NOW() as updated_at
    FROM reviews r
    LEFT JOIN (
        SELECT 
            offer_id,
            rating,
            COUNT(*) as count
        FROM reviews 
        WHERE offer_id = COALESCE(NEW.offer_id, OLD.offer_id)
        AND is_moderated = true
        GROUP BY offer_id, rating
    ) rating_counts ON r.offer_id = rating_counts.offer_id AND r.rating = rating_counts.rating
    WHERE r.offer_id = COALESCE(NEW.offer_id, OLD.offer_id)
    AND r.is_moderated = true
    GROUP BY r.offer_id
    ON CONFLICT (offer_id) 
    DO UPDATE SET
        average_rating = EXCLUDED.average_rating,
        reviews_count = EXCLUDED.reviews_count,
        rating_distribution = EXCLUDED.rating_distribution,
        updated_at = EXCLUDED.updated_at;
    
    -- If no moderated reviews exist, delete the rating record
    IF NOT EXISTS (
        SELECT 1 FROM reviews 
        WHERE offer_id = COALESCE(NEW.offer_id, OLD.offer_id) 
        AND is_moderated = true
    ) THEN
        DELETE FROM offer_ratings 
        WHERE offer_id = COALESCE(NEW.offer_id, OLD.offer_id);
    END IF;
    
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- Create triggers to automatically update offer ratings
CREATE TRIGGER trigger_update_offer_rating_on_insert
    AFTER INSERT ON reviews
    FOR EACH ROW
    EXECUTE FUNCTION update_offer_rating();

CREATE TRIGGER trigger_update_offer_rating_on_update
    AFTER UPDATE ON reviews
    FOR EACH ROW
    WHEN (OLD.rating IS DISTINCT FROM NEW.rating OR OLD.is_moderated IS DISTINCT FROM NEW.is_moderated)
    EXECUTE FUNCTION update_offer_rating();

CREATE TRIGGER trigger_update_offer_rating_on_delete
    AFTER DELETE ON reviews
    FOR EACH ROW
    EXECUTE FUNCTION update_offer_rating();