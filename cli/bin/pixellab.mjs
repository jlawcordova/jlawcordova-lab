#!/usr/bin/env node
// pixellab: the Pixel Lab CLI. `pixellab help` lists the commands.

import { main } from '../lib/cli.mjs';

// Piped into head, stdout closes early: stop printing quietly.
process.stdout.on('error', (err) => {
  if (/** @type {NodeJS.ErrnoException} */ (err).code !== 'EPIPE') throw err;
});

process.exitCode = await main(process.argv.slice(2));
