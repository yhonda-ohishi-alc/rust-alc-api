-- タスク (対応活動の管理単位)
CREATE TABLE alc_api.trouble_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES alc_api.tenants(id),
    ticket_id UUID NOT NULL REFERENCES alc_api.trouble_tickets(id) ON DELETE CASCADE,
    task_type TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'in_progress', 'done')),
    assigned_to UUID,
    due_date TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_trouble_tasks_ticket ON alc_api.trouble_tasks(ticket_id);
CREATE INDEX idx_trouble_tasks_ticket_status ON alc_api.trouble_tasks(ticket_id, status);

-- アクティビティログ (タスク内の時系列記録)
CREATE TABLE alc_api.trouble_task_activities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES alc_api.tenants(id),
    task_id UUID NOT NULL REFERENCES alc_api.trouble_tasks(id) ON DELETE CASCADE,
    body TEXT NOT NULL DEFAULT '',
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_trouble_activities_task ON alc_api.trouble_task_activities(task_id);
CREATE INDEX idx_trouble_activities_occurred ON alc_api.trouble_task_activities(task_id, occurred_at);

-- アクティビティ添付ファイル
CREATE TABLE alc_api.trouble_activity_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES alc_api.tenants(id),
    activity_id UUID NOT NULL REFERENCES alc_api.trouble_task_activities(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'application/octet-stream',
    size_bytes BIGINT NOT NULL DEFAULT 0,
    storage_key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_trouble_activity_files ON alc_api.trouble_activity_files(activity_id);

-- RLS
ALTER TABLE alc_api.trouble_tasks ENABLE ROW LEVEL SECURITY;
ALTER TABLE alc_api.trouble_task_activities ENABLE ROW LEVEL SECURITY;
ALTER TABLE alc_api.trouble_activity_files ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON alc_api.trouble_tasks
    FOR ALL USING (tenant_id = current_setting('app.current_tenant_id', true)::uuid)
    WITH CHECK (tenant_id = current_setting('app.current_tenant_id', true)::uuid);

CREATE POLICY tenant_isolation ON alc_api.trouble_task_activities
    FOR ALL USING (tenant_id = current_setting('app.current_tenant_id', true)::uuid)
    WITH CHECK (tenant_id = current_setting('app.current_tenant_id', true)::uuid);

CREATE POLICY tenant_isolation ON alc_api.trouble_activity_files
    FOR ALL USING (tenant_id = current_setting('app.current_tenant_id', true)::uuid)
    WITH CHECK (tenant_id = current_setting('app.current_tenant_id', true)::uuid);

-- GRANT
GRANT SELECT, INSERT, UPDATE, DELETE ON alc_api.trouble_tasks TO alc_api_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON alc_api.trouble_task_activities TO alc_api_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON alc_api.trouble_activity_files TO alc_api_app;
