-- タスクに「いつ発生したか」を記録するカラムを追加
ALTER TABLE alc_api.trouble_tasks ADD COLUMN occurred_at TIMESTAMPTZ;
