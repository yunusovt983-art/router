-- Create reviews table
CREATE TABLE reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    offer_id UUID NOT NULL,
    author_id UUID NOT NULL,
    rating INTEGER NOT NULL CHECK (rating >= 1 AND rating <= 5),
    text TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_moderated BOOLEAN NOT NULL DEFAULT FALSE,
    moderation_status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (moderation_status IN ('pending', 'approved', 'rejected', 'flagged'))
);

-- Create indexes for performance
CREATE INDEX idx_reviews_offer_id ON reviews (offer_id);
CREATE INDEX idx_reviews_author_id ON reviews (author_id);
CREATE INDEX idx_reviews_created_at ON reviews (created_at DESC);
CREATE INDEX idx_reviews_rating ON reviews (rating);
CREATE INDEX idx_reviews_moderation_status ON reviews (moderation_status);

-- Create composite index for common queries
CREATE INDEX idx_reviews_offer_moderated ON reviews (offer_id, is_moderated, created_at DESC);