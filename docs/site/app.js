'use strict';
const status = document.getElementById('copy-status');
async function copyText(text, button) {
  try {
    await navigator.clipboard.writeText(text);
    button.textContent = 'Copied!';
    status.textContent = 'Command copied to clipboard.';
  } catch {
    button.textContent = 'Select text';
    status.textContent = 'Clipboard unavailable. Select and copy the command manually.';
    const target = button.id === 'copy-command' ? document.getElementById('demo-command') : button.parentElement.querySelector('code');
    const range = document.createRange();
    range.selectNodeContents(target);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  }
  setTimeout(() => { button.textContent = 'Copy'; }, 2200);
}
document.querySelectorAll('pre:not(.demo-output)').forEach(pre => {
  const code = pre.querySelector('code');
  if (!code) return;
  const button = document.createElement('button');
  button.type = 'button'; button.className = 'copy'; button.textContent = 'Copy';
  button.setAttribute('aria-label', 'Copy code example');
  button.addEventListener('click', () => copyText(code.textContent, button));
  pre.append(button);
});
document.querySelectorAll('table').forEach(table => {
  const wrapper = document.createElement('div'); wrapper.className = 'table-wrap';
  wrapper.tabIndex = 0; wrapper.setAttribute('role', 'region'); wrapper.setAttribute('aria-label', 'Scrollable reference table');
  table.before(wrapper); wrapper.append(table);
});
const form = document.getElementById('demo-form');
if (form) {
  const alias = document.getElementById('device');
  const format = document.getElementById('format');
  const interval = document.getElementById('interval');
  const count = document.getElementById('count');
  const command = document.getElementById('demo-command');
  const output = document.getElementById('demo-output');
  const button = document.getElementById('copy-command');
  const quote = value => "'" + value.replaceAll("'", "'\\''") + "'";
  const measurement = { power_w: 42.6, voltage_v: null, current_a: null, energy_wh: null, today_energy_wh: 184, month_energy_wh: 3240 };
  function update() {
    const streaming = format.value !== 'json';
    document.getElementById('watch-controls').hidden = !streaming;
    interval.disabled = count.disabled = !streaming;
    alias.setCustomValidity(alias.value.trim() ? '' : 'Enter a device alias.');
    if (!form.checkValidity()) {
      button.disabled = true; command.textContent = 'Enter a device name and valid sampling settings.'; return;
    }
    button.disabled = false;
    // POSIX single-quote escaping keeps aliases literal, including $ and apostrophes.
    const device = alias.value.trim();
    const base = `denki energy ${streaming ? 'watch ' : ''}${quote(device)}`;
    command.textContent = streaming ? `${base} --interval ${Number(interval.value)} --count ${Number(count.value)} --format ${format.value} > energy.${format.value}` : `${base} --json`;
    const envelope = { schema_version: 2, command: 'energy', status: 'ok', data: { device, outlet: null, source: 'device', measurement }, error: null };
    const sample = { schema_version: 1, timestamp_unix_ms: 1791288000000, device, outlet: null, status: 'ok', source: 'device', measurement, error: null };
    const csv = value => /[,"\n\r]/.test(String(value)) ? '"' + String(value).replaceAll('"', '""') + '"' : String(value);
    if (format.value === 'json') output.textContent = JSON.stringify(envelope, null, 2);
    else if (format.value === 'jsonl') output.textContent = [sample, { ...sample, timestamp_unix_ms: sample.timestamp_unix_ms + Number(interval.value) * 1000, measurement: { ...measurement, power_w: 41.8 } }].slice(0, Number(count.value)).map(x => JSON.stringify(x)).join('\n');
    else output.textContent = 'timestamp_unix_ms,device,outlet,status,power_w,voltage_v,current_a,energy_wh,today_energy_wh,month_energy_wh,error_code,error_message\n' + [sample.timestamp_unix_ms, device, '', 'ok', 42.6, '', '', '', 184, 3240, '', ''].map(csv).join(',');
    document.getElementById('output-label').textContent = `Sample ${format.value.toUpperCase()} ${streaming ? 'records (excerpt)' : 'result'}`;
    document.getElementById('demo-note').textContent = streaming ? 'Sample excerpt only. Run the copied command to collect your own readings. The > operator replaces an existing output file.' : 'Unavailable measurements are null, not zero. One-shot JSON uses a schema-v2 envelope.';
  }
  form.addEventListener('input', update); form.addEventListener('change', update);
  form.addEventListener('submit', e => e.preventDefault());
  button.addEventListener('click', () => copyText(command.textContent, button));
  update();
}
