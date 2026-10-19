// Errors every door understands. A CommandError carries a stable `code` and
// the CLI exit code: 1 for a command that ran and failed (a missing scene, a
// site that isn't there), 2 for a usage error (bad params, unknown command).

export class CommandError extends Error {
  /**
   * @param {string} message
   * @param {{ code?: string, exitCode?: number, details?: unknown }} [options]
   */
  constructor(message, { code = 'error', exitCode = 1, details } = {}) {
    super(message);
    this.name = 'CommandError';
    this.code = code;
    this.exitCode = exitCode;
    if (details !== undefined) this.details = details;
  }

  toJSON() {
    return { code: this.code, message: this.message, ...(this.details !== undefined ? { details: this.details } : {}) };
  }
}

/** Bad params, an unknown command or flag: exit code 2. */
export class UsageError extends CommandError {
  /** @param {string} message @param {{ details?: unknown }} [options] */
  constructor(message, { details } = {}) {
    super(message, { code: 'usage', exitCode: 2, details });
    this.name = 'UsageError';
  }
}

/** A scene, object or file that doesn't exist. */
export class NotFoundError extends CommandError {
  /** @param {string} message */
  constructor(message) {
    super(message, { code: 'not_found' });
    this.name = 'NotFoundError';
  }
}

/** The site checkout can't be found or doesn't look like the site. */
export class SiteError extends CommandError {
  /** @param {string} message */
  constructor(message) {
    super(message, { code: 'site' });
    this.name = 'SiteError';
  }
}
