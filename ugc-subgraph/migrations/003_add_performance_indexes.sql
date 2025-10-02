-- Add indexes for performance optimization and N+1 query prevention

-- Index for batch loading reviews by IDs
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reviews_id_batch 
ON reviews (id) 
WHERE is_moderated = true;

-- Index for batch loading reviews by offer IDs (already exists but ensure it's optimized)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reviews_offer_id_moderated_created 
ON reviews (offer_id, created_at DESC) 
WHERE is_moderated = true;

-- Index for batch loading reviews by author IDs
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reviews_author_id_moderated_created 
ON reviews (author_id, created_at DESC) 
WHERE is_moderated = true;

-- Composite index for filtering by offer and moderation status
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reviews_offer_moderated_rating 
ON reviews (offer_id, is_moderated, rating, created_at DESC);

-- Composite index for filtering by author and moderation status
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reviews_author_moderated_rating 
ON reviews (author_id, is_moderated, rating, created_at DESC);

-- Index for offer ratings batch loading
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_offer_ratings_offer_id_updated 
ON offer_ratings (offer_id, updated_at DESC);

-- Index for moderation queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reviews_moderation_status_created 
ON reviews (moderation_status, created_at DESC) 
WHERE is_moderated = false;

-- Index for rating distribution queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reviews_rating_offer_moderated 
ON reviews (rating, offer_id) 
WHERE is_moderated = true;

-- Partial index for pending moderation
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reviews_pending_moderation 
ON reviews (created_at DESC) 
WHERE moderation_status = 'pending' AND is_moderated = false;

-- Index for cursor-based pagination
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reviews_cursor_pagination 
ON reviews (created_at DESC, id DESC) 
WHERE is_moderated = true;

-- Index for offer statistics calculation
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reviews_offer_stats 
ON reviews (offer_id, rating) 
WHERE is_moderated = true;

-- Add comments for documentation
COMMENT ON INDEX idx_reviews_id_batch IS 'Optimizes batch loading of reviews by IDs';
COMMENT ON INDEX idx_reviews_offer_id_moderated_created IS 'Optimizes loading reviews by offer with moderation filter';
COMMENT ON INDEX idx_reviews_author_id_moderated_created IS 'Optimizes loading reviews by author with moderation filter';
COMMENT ON INDEX idx_reviews_offer_moderated_rating IS 'Optimizes complex filtering by offer, moderation, and rating';
COMMENT ON INDEX idx_reviews_author_moderated_rating IS 'Optimizes complex filtering by author, moderation, and rating';
COMMENT ON INDEX idx_offer_ratings_offer_id_updated IS 'Optimizes batch loading of offer ratings';
COMMENT ON INDEX idx_reviews_moderation_status_created IS 'Optimizes moderation queue queries';
COMMENT ON INDEX idx_reviews_rating_offer_moderated IS 'Optimizes rating distribution calculations';
COMMENT ON INDEX idx_reviews_pending_moderation IS 'Optimizes pending moderation queries';
COMMENT ON INDEX idx_reviews_cursor_pagination IS 'Optimizes cursor-based pagination';
COMMENT ON INDEX idx_reviews_offer_stats IS 'Optimizes offer statistics calculations';