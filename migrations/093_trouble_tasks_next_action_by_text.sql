-- next_action_by を UUID → TEXT に変更 (従業員名テキストも受け入れ可能にする)
-- 既存の UUID 値は TEXT にキャストされる
ALTER TABLE alc_api.trouble_tasks
    ALTER COLUMN next_action_by TYPE TEXT USING next_action_by::text;
