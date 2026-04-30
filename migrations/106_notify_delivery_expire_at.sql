-- notify_deliveries.expire_at: 配信ごとの閲覧期限 (任意日時)
-- read_tracker が expire_at > NOW() を判定し、有効なら R2 presigned URL に redirect
ALTER TABLE alc_api.notify_deliveries
    ADD COLUMN expire_at TIMESTAMPTZ;

-- 既存行は created_at + 7 days で初期化
UPDATE alc_api.notify_deliveries
    SET expire_at = created_at + INTERVAL '7 days'
    WHERE expire_at IS NULL;

ALTER TABLE alc_api.notify_deliveries
    ALTER COLUMN expire_at SET NOT NULL;

ALTER TABLE alc_api.notify_deliveries
    ALTER COLUMN expire_at SET DEFAULT (NOW() + INTERVAL '7 days');

-- mark_delivery_read を拡張: r2_key + expire_at も返す
-- read_tracker が presigned URL を組み立てるのに必要
-- 注: 既存の mark_delivery_read (migration 071) は戻り値型が違うので
-- CREATE OR REPLACE は使えない (Pg 42P13)。DROP してから CREATE する。
DROP FUNCTION IF EXISTS alc_api.mark_delivery_read(UUID);

CREATE FUNCTION alc_api.mark_delivery_read(p_read_token UUID)
RETURNS TABLE(
    document_id UUID,
    tenant_id UUID,
    r2_key TEXT,
    expire_at TIMESTAMPTZ
)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = alc_api
AS $$
BEGIN
    UPDATE alc_api.notify_deliveries
    SET read_at = NOW()
    WHERE read_token = p_read_token AND read_at IS NULL;

    RETURN QUERY
    SELECT d.document_id, d.tenant_id, doc.r2_key, d.expire_at
    FROM alc_api.notify_deliveries d
    JOIN alc_api.notify_documents doc ON doc.id = d.document_id
    WHERE d.read_token = p_read_token;
END;
$$;

GRANT EXECUTE ON FUNCTION alc_api.mark_delivery_read(UUID) TO alc_api_app;
