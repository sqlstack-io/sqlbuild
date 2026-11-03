-- Crash successive supervisors while a stopped child's terminal publication is rejected.
-- Claims and the inherited ownership fence must survive until the terminal fact commits.
CREATE TRIGGER reject_terminal BEFORE UPDATE OF status ON sqb_runs
WHEN NEW.status = 'failed'
BEGIN SELECT RAISE(ABORT, 'synthetic terminal publication failure'); END;
