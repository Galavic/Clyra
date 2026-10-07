// Usage: node scripts/export-dot-animations.cjs downloaded-dots-lab.html frames.ndjson
// Extracts only the site's pure drawing/choreography functions. No browser code runs.
const fs = require('fs');
const vm = require('vm');
const html = fs.readFileSync(process.argv[2], 'utf8');
function section(start, end) {
  const a = html.indexOf(start);
  const b = html.indexOf(end, a);
  if (a < 0 || b < 0) throw new Error(`Dots Lab renderer changed: ${start}`);
  return html.slice(a, b);
}
const source = [
  section('const TAU=', 'const DEFAULT='),
  'const S={}; const EXP=512; const pageInk="#FFFFFF";',
  section('const shapeOf=', '/* ================= UI ================= */'),
  section('class SvgCtx', '/* archive zip'),
  section('const easeIO=', 'const hero='),
  'globalThis.renderer={FAMILIES,STATES,CYC,SvgCtx,drawBlob,choreo};',
].join('\n');
class Path2D { moveTo() {} lineTo() {} closePath() {} }
const sandbox = { Path2D, matchMedia: () => ({matches:false}) };
vm.runInNewContext(source, sandbox, {timeout:10000});
const r = sandbox.renderer;
const output = fs.openSync(process.argv[3], 'w');
try {
  const states = ['idle', 'think', 'alert', 'error'];
  r.FAMILIES.flatMap(f => f.items).forEach((preset, index) => {
    states.forEach(state => {
      const st = r.STATES.find(s => s.id === state);
      const duration = r.CYC[state];
      const count = Math.round(duration * 20);
      const frames = Array.from({length:count}, (_, i) => {
        const phase = i / count;
        const motion = r.choreo(state, phase, phase * duration);
        const context = new r.SvgCtx();
        r.drawBlob(context, 512, preset, {
          ...motion, t:phase*duration, p:phase, st,
          look:{x:(motion.look||[0,0])[0],y:(motion.look||[0,0])[1]},scale:1,
        }, {hero:true,u:.3,cy:.58});
        // GPUI rasterizes SVGs at 2× resolution; the asset tool's .25 scale
        // turns this 256px viewport into the final 128px transparent frame.
        return context.svg(512).replace('width="512" height="512"', 'width="256" height="256"');
      });
      fs.writeSync(output, JSON.stringify({index,state,duration,frames})+'\n');
      if (state === 'idle') {
        // Match the loading/reduced-motion pose to the animation's first frame.
        fs.writeFileSync(`crates/ui/assets/dots/dot-${index}.svg`, frames[0].replace('width="256" height="256"', 'width="512" height="512"'));
      }
    });
  });
} finally { fs.closeSync(output); }
console.log('Exported 19 presets × 4 original animation states at 20 fps.');
