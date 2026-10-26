-- Persist the latest accepted auth time; restarting cannot reset the rollback boundary.
CREATE TABLE security_clock (
    singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK(singleton),
    high_water BIGINT NOT NULL CHECK(high_water >= 0)
);
INSERT INTO security_clock(singleton,high_water) VALUES(true,0);
