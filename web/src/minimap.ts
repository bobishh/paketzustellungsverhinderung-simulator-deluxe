type Point = [number, number];
type Road = { points: Point[]; width: number };
let roads: Road[] = [];
const canvas = document.querySelector<HTMLCanvasElement>('#radar')!;
const ctx = canvas.getContext('2d')!;
window.addEventListener('paket-map-roads', ((e: CustomEvent<Road[]>) => { roads = e.detail; }) as EventListener);
window.addEventListener('paket-radar', ((e: CustomEvent<{ position: Point; heading: number; target?: Point; van?: Point; dogs: Point[]; people: Point[]; peers: Point[] }>) => {
  const s = e.detail, half = 80, scale = 480 / (half * 2);
  const xy = (p: Point): Point => [240 + (p[0] - s.position[0]) * scale, 240 + (p[1] - s.position[1]) * scale];
  ctx.clearRect(0, 0, 480, 480); ctx.fillStyle = '#233b2e'; ctx.fillRect(0, 0, 480, 480);
  ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  for (const road of roads) {
    ctx.beginPath(); road.points.forEach((p, i) => { const [x,y] = xy(p); if (i) ctx.lineTo(x,y); else ctx.moveTo(x,y); });
    ctx.lineWidth = Math.max(3, road.width * scale); ctx.strokeStyle = '#7d8775'; ctx.stroke();
  }
  const dot = (p: Point, color: string, size: number, square = false, clamp = false) => {
    let [x,y] = xy(p); if(clamp){x=Math.max(12,Math.min(468,x));y=Math.max(12,Math.min(468,y));}
    ctx.fillStyle = color;ctx.strokeStyle='#17231e';ctx.lineWidth=3;ctx.beginPath();
    if(square)ctx.rect(x-size,y-size,size*2,size*2);else ctx.arc(x,y,size,0,Math.PI*2);ctx.fill();ctx.stroke();
  };
  for(const p of s.dogs)dot(p,'#efba76',5);
  for(const p of s.people)dot(p,'#a7ddca',6);
  for(const p of s.peers)dot(p,'#92b9ed',7);
  if(s.van)dot(s.van,'#ffdb84',9,true,true);
  if(s.target)dot(s.target,'#ff8436',11,false,true);
  ctx.save();ctx.translate(240,240);ctx.rotate(-s.heading);ctx.beginPath();ctx.moveTo(0,-16);ctx.lineTo(11,11);ctx.lineTo(0,5);ctx.lineTo(-11,11);ctx.closePath();ctx.fillStyle='#fff9e8';ctx.fill();ctx.strokeStyle='#102419';ctx.lineWidth=3;ctx.stroke();ctx.restore();
  ctx.beginPath();ctx.arc(240,240,90*scale/2,0,Math.PI*2);ctx.strokeStyle='#ffffff20';ctx.lineWidth=1;ctx.stroke();
}) as EventListener);

export {};
