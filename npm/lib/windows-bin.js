const { existsSync, realpathSync, readFileSync, writeFileSync } = require('fs');
const { dirname, join, relative, resolve } = require('path');
const { name: PACKAGE_NAME, bin: COMMANDS } = require('../package.json');

function rewriteShims(destination, command, binary, targets) {
  const matches = (contents) => targets.some((target) =>
    contents.includes(`${target}"`) || contents.includes(`${target}'`));
  const shim = join(destination, `${command}.cmd`);
  try {
    const contents = readFileSync(shim, 'utf8').replace(/\\/g, '/');
    if (!matches(contents)) return false;
    writeFileSync(shim, `@echo off\r\n"${binary}" %*\r\nexit /b %errorlevel%\r\n`);
    const powershell = join(destination, `${command}.ps1`);
    if (existsSync(powershell) && matches(readFileSync(powershell, 'utf8').replace(/\\/g, '/'))) {
      writeFileSync(powershell, `& '${binary.replace(/'/g, "''")}' @args\r\nexit $LASTEXITCODE\r\n`);
    }
    return true;
  } catch (error) {
    if (error.code !== 'ENOENT') process.stderr.write(`agent-desktop: Native command optimization skipped: ${error.message}\n`);
    return false;
  }
}

module.exports = function linkWindowsBin(directory, binaryName) {
  const binary = resolve(directory, binaryName);
  if (!existsSync(binary) || /[%!\r\n"]/.test(binary)) return false;
  const entry = join(directory, 'agent-desktop');
  let ancestor = directory;
  let linked = false;
  for (;;) {
    for (const destination of [ancestor, join(ancestor, '.bin')]) {
      const targets = [relative(destination, entry).replace(/\\/g, '/')];
      const alias = join(ancestor, PACKAGE_NAME, 'bin', 'agent-desktop');
      try {
        if (existsSync(alias) && realpathSync(alias) === realpathSync(entry)) {
          targets.push(relative(destination, alias).replace(/\\/g, '/'));
        }
      } catch {}
      for (const command of Object.keys(COMMANDS)) {
        if (rewriteShims(destination, command, binary, targets)) linked = true;
      }
    }
    const parent = dirname(ancestor);
    if (parent === ancestor) return linked;
    ancestor = parent;
  }
};
