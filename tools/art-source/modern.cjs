'use strict';
// Original fictional vehicle exteriors and industrial props. All mobile templates face +Z.
const halfPi=Math.PI*.5;
const materials=[
 ['graphite','#343c42','metal'],['underbody','#2b3336','metal'],['rubber','#303533','cloth'],
 ['petrol','#49776f','metal'],['patrolKhaki','#b6ac8c','metal'],['edgeTan','#c0b79a','metal'],
 ['glassBlue','#456d80','glass'],['glassBright','#83acb5','glass'],['lampWhite','#eee5c5','light'],
 ['amber','#c7904f','light'],['brake','#974844','glass'],['convoy','#6b887a','metal'],
 ['convoyShade','#515e58','metal'],['composite','#6e766b','metal'],['compositeEdge','#859084','metal'],
 ['optic','#4e8587','glass'],['freight','#628996','metal'],['freightEdge','#87a6ab','metal'],
 ['industrial','#a9b6b1','metal'],['asphalt','#6f7370','asphalt'],['railYellow','#e6b85c','metal'],
 ['stripeWhite','#c3c4ac','plaster'],['charred','#494d47','metal'],['slateBody','#527882','metal'],['slateArmor','#819c9d','metal'],['fieldBlue','#448da8','metal'],['signalBlue','#92c4ca','light'],['coyote','#a39273','metal'],['dune','#c5a982','metal'],['clay','#a96749','metal'],['fieldRed','#b35e50','metal'],['gaiter','#4d5450','cloth'],['runningLamp','#edac54','light'],['readyLamp','#98cc94','light'],['standbyLamp','#728783','glass'],['ochreDust','#938772','cloth']
];
function wheel(r,x,y,z,radius,thick,side=Math.sign(x)||1){
  const outer=x+side*thick*.5;
  r.cylinder(x,y,z,radius,radius,thick,'rubber',14,0,0,halfPi);
  r.cylinder(outer+side*.008,y,z,radius*.71,radius*.71,.055,'graphite',12,0,0,halfPi);
  r.cylinder(outer+side*.041,y,z,radius*.48,radius*.48,.054,'convoy',8,0,0,halfPi);
  r.cylinder(outer+side*.078,y,z,radius*.19,radius*.21,.074,'graphite',8,0,0,halfPi);
  for(let i=0;i<7;i++){
    const a=i*2*Math.PI/7,yy=y+Math.sin(a)*radius*.34,zz=z+Math.cos(a)*radius*.34;
    r.cylinder(outer+side*.074,yy,zz,.026,.026,.035,'industrial',5,0,0,halfPi);
  }
}
function panel(r,x,y,z,w,h,d,mat='composite',yaw=0,pitch=0,roll=0){r.box(x,y,z,w,h,d,mat,yaw,pitch,roll);}
function axlePair(r,w,z,rad){
  for(const s of [-1,1])wheel(r,s*w,rad,z,rad,rad*.62);
}
function lightBar(r,x,y,z,w,front=true){
  const sign=front?1:-1;
  panel(r,x,y,z,w,.118,.061,'graphite');
  panel(r,x,y,z+sign*.036,w*.69,.073,.026,front?'lampWhite':'brake');
  panel(r,x+Math.sign(x)*w*.40,y,z+sign*.040,w*.13,.071,.024,'amber');
}
function glazed(r,x,y,z,w,h,depth,rotation=0,tilt=0){
  panel(r,x,y,z,w+.053,h+.056,depth+.021,'graphite',rotation,tilt);
  panel(r,x+Math.sin(rotation)*.018,y+.002,z+Math.cos(rotation)*.018,w,h,depth+.032,'glassBlue',rotation,tilt);
}
function patrol(r,mobile=false){
  panel(r,0,.49,-.01,1.65,.35,3.32,'underbody');
  if(!mobile)for(const z of [-1.12,1.16])axlePair(r,.80,z,.431);
  panel(r,0,.85,0,1.72,.51,3.62,'patrolKhaki');
  panel(r,0,1.10,1.04,1.64,.23,1.05,'edgeTan',0,-.09);
  panel(r,0,1.40,-.35,1.57,.64,1.91,'patrolKhaki');
  panel(r,0,1.78,-.42,1.52,.15,1.59,'edgeTan');
  panel(r,0,1.43,.63,1.51,.53,.12,'graphite',0,.39);
  for(const s of [-1,1]){
    panel(r,s*.384,1.45,.688,.702,.405,.038,'glassBlue',0,.39);
    for(const z of [-.82,.10]){
      panel(r,s*.806,1.465,z,.042,.35,.69,'graphite');
      panel(r,s*.833,1.477,z,.025,.272,.58,'glassBlue');
      panel(r,s*.863,1.028,z,.10,.35,.76,'patrolKhaki',0,0,s*.09);
      panel(r,s*.876,1.272,z-.17,.041,.035,.137,'graphite');
    }
    panel(r,s*.89,.66,-.14,.16,.068,1.46,'graphite');
    panel(r,s*.82,1.03,1.12,.39,.13,.99,'edgeTan');
    panel(r,s*.82,1.04,-1.13,.39,.13,.98,'edgeTan');
    panel(r,s*.90,1.50,.53,.18,.15,.12,'graphite');
    lightBar(r,s*.61,.94,1.816,.37);
    lightBar(r,s*.67,.99,-1.804,.24,false);
  }
  panel(r,0,.91,1.835,.70,.22,.055,'graphite');
  for(let i=-3;i<=3;i++)panel(r,i*.091,.91,1.879,.035,.18,.026,'industrial');
  panel(r,0,.626,1.84,1.80,.136,.095,'graphite');
  panel(r,0,.62,-1.83,1.81,.13,.095,'graphite');
  panel(r,0,1.35,-1.336,.92,.30,.075,'glassBlue');
  panel(r,-.46,1.55,-1.315,.37,.18,.22,'patrolKhaki');
  r.cylinder(.12,1.885,-.37,.30,.30,.074,'graphite',10);
  panel(r,.12,1.938,-.37,.40,.055,.40,'patrolKhaki');
  r.beam(-.59,1.78,-.93,-.64,2.50,-1.03,.022,.022,'graphite');
}
function cargo(r,mobile=false,team=""){
  panel(r,0,.60,-.15,1.64,.29,5.32,'underbody');
  if(!mobile)for(const z of [1.93,-.93,-2.05])axlePair(r,1.02,z,.523);
  panel(r,0,1.29,1.56,2.14,.91,1.84,'convoy');
  panel(r,0,1.98,1.40,2.05,.61,1.45,'convoy');
  panel(r,0,2.32,1.38,2.10,.11,1.53,'convoyShade');
  panel(r,0,1.96,2.22,1.99,.54,.08,'graphite',0,.18);
  for(const s of [-1,1]){
    panel(r,s*.497,1.97,2.267,.908,.424,.057,'glassBlue',0,.18);
    panel(r,s*1.044,2.018,1.49,.05,.35,.82,'glassBlue');
    panel(r,s*1.06,1.455,1.54,.075,.54,.94,'convoy');
    panel(r,s*1.094,1.67,1.20,.04,.042,.20,'graphite');
    panel(r,s*1.13,.87,1.03,.29,.09,.78,'industrial');
    r.beam(s*1.04,2.15,2.1,s*1.27,2.19,2.0,.035,.035,'graphite');
    panel(r,s*1.28,2.13,2.0,.088,.22,.12,'graphite');
    lightBar(r,s*.77,1.17,2.532,.38);
    lightBar(r,s*.88,1.08,-2.85,.31,false);
  }
  panel(r,0,1.47,2.52,1.83,.33,.18,'convoyShade');
  for(let i=0;i<4;i++)panel(r,0,1.39+i*.065,2.62,1.4,.032,.026,'graphite');
  panel(r,0,.83,2.59,2.27,.184,.20,'industrial');
  panel(r,0,1.11,-1.01,2.20,.17,3.87,'convoyShade');
  if(team==='red'){
    // Rounded canvas body that immediately separates the red convoy from blue ISO trucks.
    panel(r,0,1.47,-1.04,2.13,.64,3.64,'convoyShade');
    r.cylinder(0,1.84,-1.04,.91,.91,3.57,'gaiter',12,0,halfPi);
    for(const x of [-.945,.945])panel(r,x,1.85,-1.04,.27,.45,3.55,'gaiter',0,0,-Math.sign(x)*.24);
    for(let i=0;i<7;i++){
      const z=-2.69+i*.53;r.cylinder(0,1.84,z,.922,.922,.049,'convoyShade',12,0,halfPi);
      for(const side of [-1,1])r.beam(side*1.035,1.89,z,side*1.065,1.25,z,.046,.044,'edgeTan');
    }
    panel(r,0,1.49,-2.92,2.10,.58,.10,'convoy');
    panel(r,0,1.15,-2.97,2.21,.16,.18,'graphite');
    return;
  }
  panel(r,0,1.79,-1.04,2.13,1.24,3.64,'freight');
  for(const s of [-1,1]){
    for(let i=0;i<12;i++)panel(r,s*1.081,1.77,-2.70+i*.30,.045,1.06,.033,'freightEdge');
    panel(r,s*1.073,2.39,-1.04,.075,.11,3.64,'graphite');
  }
  panel(r,0,2.44,-1.04,2.18,.09,3.70,'freightEdge');
  for(const s of [-1,1]){
    panel(r,s*.53,1.81,-2.891,.984,1.09,.063,'freightEdge');
    for(const x of [s*.28,s*.74])panel(r,x,1.81,-2.936,.031,.97,.026,'industrial');
  }
  panel(r,0,1.15,-2.97,2.21,.16,.18,'graphite');
}
function mbt(r,mobile=false){
  panel(r,0,.74,-.19,2.37,.70,4.96,'convoyShade');
  panel(r,0,.80,2.34,2.31,.56,.83,'composite',0,-.40);
  panel(r,0,.93,2.66,1.56,.07,.16,'convoyShade');
  panel(r,0,1.17,.04,2.68,.56,5.03,'composite');
  panel(r,0,1.24,2.25,2.51,.25,1.12,'compositeEdge',0,.27);
  panel(r,0,1.48,-1.69,2.50,.095,1.70,'convoyShade');
  for(let k=0;k<9;k++)panel(r,0,1.54,-2.37+.15*k,1.67,.026,.055,'graphite');
  for(const s of [-1,1]){
    panel(r,s*1.13,.58,-.06,.54,.72,5.33,'rubber');
    if(!mobile)for(let i=0;i<7;i++){
      const zz=-2.13+i*.69;
      r.cylinder(s*1.38,.52,zz,.336,.336,.083,'graphite',12,0,0,halfPi);
      r.cylinder(s*1.431,.52,zz,.248,.248,.041,'convoy',10,0,0,halfPi);
      r.cylinder(s*1.461,.52,zz,.094,.094,.038,'graphite',8,0,0,halfPi);
    }
    for(let i=0;i<5;i++){
      const zz=-1.98+i*.91;
      panel(r,s*1.37,1.06,zz,.22,.56,.87,'composite',0,0,s*.097);
      panel(r,s*1.50,1.24,zz,.04,.067,.61,'compositeEdge');
    }
    panel(r,s*1.11,1.392,2.52,.34,.11,.17,'graphite');
    panel(r,s*1.11,1.412,2.622,.22,.066,.034,'lampWhite');
    panel(r,s*1.16,1.12,-2.64,.20,.12,.08,'brake');
  }
  r.cylinder(0,1.51,.34,.88,.88,.145,'graphite',12);
}
function turret(r,mobile=false){
  // Turret origin lies at 1.73m above the hull. Local +Z follows the fire direction.
  r.cylinder(0,-.06,0,.91,.96,.28,'convoyShade',8,.39);
  panel(r,0,.13,-.04,1.79,.55,1.82,'composite');
  for(const s of [-1,1]){
    panel(r,s*.80,.19,.54,.68,.47,.81,'compositeEdge',s*.29,0,s*.17);
    panel(r,s*1.045,.22,-.24,.27,.43,.91,'composite',s*.07,0,s*.14);
    panel(r,s*.78,.14,-1.03,.49,.44,.53,'convoyShade');
    for(let i=0;i<3;i++)r.cylinder(s*(.78+i*.10),.46,-.17,.083,.080,.17,'graphite',7,0,s*.30);
  }
  panel(r,0,.23,.96,.51,.40,.36,'graphite');
  if(!mobile)r.group(0,.24,.96,0,()=>mbtBarrel(r));
  r.cylinder(.40,.461,-.36,.30,.29,.075,'graphite',10);
  panel(r,-.31,.55,-.27,.39,.29,.40,'graphite');
  panel(r,-.31,.585,-.044,.28,.115,.041,'optic');
  panel(r,.68,.52,.35,.31,.24,.35,'convoyShade');
  panel(r,.68,.54,.535,.24,.12,.027,'optic');
  r.beam(-.72,.39,-.82,-.76,1.09,-.94,.022,.022,'graphite');
}
function streetLamp(r,p){
  const h=p.h||6.1;r.cylinder(0,h*.47,0,.105,.064,h*.94,'industrial',8);
  r.cylinder(0,.27,0,.178,.14,.54,'graphite',8);
  r.beam(0,h*.91,0,-.37,h*.99,0,.09,.09,'industrial');r.beam(-.36,h*.99,0,-1.41,h*1.01,0,.07,.07,'industrial');
  panel(r,-1.38,h,0,.69,.12,.26,'convoyShade');panel(r,-1.38,h-.066,0,.53,.024,.18,'lampWhite');
}
function telecom(r,site,drawLegacyMast){
  drawLegacyMast(r,site);
  r.group(site.x,0,site.z,0,()=>{
    for(let i=0;i<3;i++){
      const a=i*2*Math.PI/3;
      r.group(Math.sin(a)*.46,9.8,Math.cos(a)*.46,-a,()=>{
        panel(r,0,.32,0,.29,1.38,.18,'industrial');panel(r,0,.32,.115,.21,1.11,.04,'stripeWhite');
      });
    }
    panel(r,1.15,.82,.3,.65,1.59,.56,'industrial');
    panel(r,1.15,.96,.595,.48,1.00,.038,'convoyShade');
    for(let i=0;i<5;i++)panel(r,1.15,.68+i*.095,.627,.33,.025,.028,'industrial');
  });
}
const objects={patrol4x4:patrol,cargo6x6:cargo,scout_buggy:buggy,apc,mbt,mbt_turret:turret,apc_turret:apcTurret,mbt_barrel:mbtBarrel,apc_barrel:apcBarrel};
function wreck(r,kind){
  const heavy=kind==='mbt',cargoType=kind==='cargo6x6',carrier=kind==='apc';
  const w=heavy?2.9:carrier?2.68:cargoType?2.3:1.84,d=heavy?5.82:carrier?6.24:cargoType?6.1:3.6;
  panel(r,0,.38,0,w*.80,.59,d*.74,'charred',0,-.035,.039);
  panel(r,w*.06,.69,d*.12,w*.59,.28,d*.39,'oxide',.14,0,-.08);
  for(const s of [-1,1]){
    if(heavy){
      panel(r,s*w*.39,.32,0,w*.20,.41,d*.88,'graphite');
      for(let i=0;i<5;i++)r.cylinder(s*w*.485,.30,-d*.33+i*d*.161,.235,.235,.11,'charred',9,0,0,halfPi);
    }else{
      for(const f of (carrier?[-.34,-.13,.13,.34]:cargoType?[-.34,-.16,.31]:[-.32,.32]))r.cylinder(s*w*.425,.28,d*f,.28,.30,.22,'charred',9,0,0,halfPi);
      r.beam(s*w*.32,.60,-d*.24,s*w*.39,1.04,d*.10,.095,.086,'graphite');
    }
  }
  if(heavy)panel(r,-.24,.94,-.23,1.26,.39,1.46,'charred',.59,0,.18);
  if(cargoType)for(const s of [-1,1])panel(r,s*.86,1.00,-1.25,.10,.39,2.32,'freight',-.11*s,0,.23*s);
}
function shippingContainer(r,p){
  const {w,d,h}=p,paint=p.x>0?'freight':'convoy';
  panel(r,0,h*.5,0,w,h,d,paint);
  for(const side of [-1,1]){
    for(const y of [.09,h-.09])panel(r,0,y,side*d*.506,w,.125,.083,'freightEdge');
    for(let i=0;i<22;i++)panel(r,-w*.47+i*w*.94/21,h*.50,side*d*.505,.06,h*.90,.060,'freightEdge');
    panel(r,side*w*.506,h*.50,0,.11,h,d*.95,'freightEdge');
    for(const z of [-d*.27,d*.27])panel(r,side*w*.517,h*.50,z,.048,h*.89,.055,'industrial');
    panel(r,-w*.34,h*.78,side*d*.529,.67,.32,.025,'industrial');
    for(let i=0;i<3;i++)panel(r,-w*.364+i*.10,h*.79,side*d*.541,.044,.16,.023,'graphite');
  }
}
function tankBattery(r,p){
  const radius=Math.min(p.w,p.d)*.49,h=p.h;
  r.cylinder(0,.18,0,radius,radius,.36,'graphite',18);r.cylinder(0,h*.48,0,radius*.95,radius*.95,h*.9,'industrial',20);
  r.cylinder(0,h*.92,0,radius*.96,radius*.52,h*.12,'industrial',20);
  for(let k=0;k<5;k++)r.cylinder(0,h*(.10+.17*k),0,radius*.965,radius*.965,.065,'convoyShade',20);
  for(const side of [-1,1])panel(r,side*.20,h*.48,radius*.96,.057,h*.89,.093,'railYellow');
  for(let i=0;i<13;i++)panel(r,0,.40+i*h*.063,radius*.99,.46,.031,.046,'industrial');
  panel(r,0,h*.55,-radius*.956,.87,.54,.062,'railYellow');
}

// Wheel coordinates and radius are mirrored in world_vehicles.rs. Static wrecks retain fixed wheels.
function buggy(r,mobile=false){
  panel(r,0,.53,0,1.44,.22,2.94,'underbody');
  panel(r,0,.70,0,1.47,.21,2.64,'patrolKhaki');
  if(!mobile)for(const z of [-1.05,1.06])axlePair(r,.84,z,.426);
  panel(r,0,.91,1.04,1.36,.18,.94,'compositeEdge',0,-.13);
  panel(r,0,1.17,-.88,1.32,.49,.63,'convoyShade');
  for(const s of [-1,1]){
    panel(r,s*.31,.90,-.02,.43,.13,.47,'gaiter');
    panel(r,s*.31,1.12,-.29,.43,.43,.13,'gaiter',0,-.11);
    r.beam(s*.67,.63,.85,s*.64,1.69,.28,.076,.08,'graphite');
    r.beam(s*.64,1.69,.28,s*.64,1.65,-.69,.076,.08,'graphite');
    r.beam(s*.64,1.65,-.69,s*.67,.77,-1.37,.079,.08,'graphite');
    r.beam(s*.72,.78,.63,s*.72,.99,-.58,.07,.07,'industrial');
    panel(r,s*.77,.92,1.05,.28,.075,.75,'patrolKhaki');
    panel(r,s*.77,.91,-1.10,.28,.075,.73,'patrolKhaki');
    lightBar(r,s*.52,.90,1.50,.28); lightBar(r,s*.54,.76,-1.475,.22,false);
  }
  panel(r,0,1.68,-.24,1.44,.076,1.10,'convoyShade');
  panel(r,0,1.48,.37,1.20,.30,.036,'glassBlue',0,.43);
  panel(r,0,1.697,.23,1.04,.12,.09,'graphite');
  for(let i=0;i<4;i++)panel(r,-.405+.27*i,1.697,.285,.18,.056,.034,'lampWhite');
  panel(r,0,.67,1.52,1.59,.105,.09,'graphite');
  r.beam(-.6,1.64,-.7,.60,.89,-1.06,.045,.045,'industrial');
  wheel(r,0,1.12,-1.42,.34,.16);
}
function apc(r,mobile=false){
  panel(r,0,.82,-.02,2.28,.57,5.64,'underbody');
  panel(r,0,1.22,0,2.48,.66,5.65,'composite');
  panel(r,0,1.70,-.22,2.10,.49,4.88,'convoy');
  panel(r,0,1.57,2.30,2.14,.32,1.15,'compositeEdge',0,.31);
  panel(r,0,1.97,-.24,1.85,.10,4.20,'compositeEdge');
  for(const side of [-1,1]){
    panel(r,side*1.125,1.65,-.26,.181,.48,4.66,'compositeEdge',0,0,-side*.38);
    for(const z of [-2.08,-.74,.68,2.03]){
      if(!mobile)wheel(r,side*1.19,.523,z,.523,.31);
      panel(r,side*1.228,1.28,z,.22,.27,1.22,'composite',0,0,side*.14);
    }
    for(const z of [-1.78,-.88,.12]){panel(r,side*1.135,1.76,z,.065,.20,.52,'graphite');panel(r,side*1.174,1.77,z,.035,.090,.36,'glassBright');}
    panel(r,side*.51,1.81,2.12,.75,.14,.065,'glassBlue',0,.35);
    lightBar(r,side*.93,1.24,2.866,.29);
    lightBar(r,side*.98,1.20,-2.83,.23,false);
  }
  for(const s of [-1,1]){
    panel(r,s*.45,1.38,-2.851,.84,.80,.07,'convoyShade');
    r.beam(s*.80,1.70,-2.905,s*.80,1.1,-2.905,.036,.038,'industrial');
    r.cylinder(s*.48,2.049,-.82,.35,.35,.077,'graphite',12);
  }
  for(const s of [-1,1]){
    panel(r,s*.47,1.962,1.61,.71,.24,.22,'graphite',0,.18);
    panel(r,s*.47,1.968,1.743,.55,.097,.04,'glassBright');
  }
  if(!mobile)r.group(0,2.,.35,0,()=>apcTurret(r));
}
function apcTurret(r,mobile=false){
  r.cylinder(0,.08,0,.41,.39,.18,'convoyShade',10);
  panel(r,0,.31,0,.51,.34,.61,'composite');panel(r,-.10,.36,.33,.21,.112,.069,'optic');
  if(!mobile)r.group(.12,.26,.22,0,()=>apcBarrel(r));
}
function mbtBarrel(r){
  r.cylinder(0,0,1.18,.108,.078,2.19,'composite',14,0,halfPi);
  r.cylinder(0,0,.54,.14,.123,.61,'convoyShade',14,0,halfPi);
  // Overlapping cuffs break up the long outline and stay readable from the optics.
  for(const z of [.21,.48,.79,1.64])r.cylinder(0,0,z,.142,.14,.057,'compositeEdge',14,0,halfPi);
  r.cylinder(0,0,2.19,.099,.095,.225,'graphite',14,0,halfPi);
  r.cylinder(0,0,2.308,.068,.068,.014,'underbody',14,0,halfPi);
  r.cylinder(.265,-.065,.46,.035,.037,.95,'graphite',10,0,halfPi);
  r.cylinder(.265,-.065,.87,.044,.044,.13,'compositeEdge',10,0,halfPi);
}
function apcBarrel(r){
  r.cylinder(0,0,.43,.056,.043,.99,'graphite',10,0,halfPi);
  for(const z of [.14,.22,.30])r.cylinder(0,0,z,.067,.067,.034,'convoyShade',10,0,halfPi);
  r.cylinder(0,0,.87,.062,.057,.15,'compositeEdge',10,0,halfPi);
  r.cylinder(0,0,.949,.038,.038,.012,'underbody',10,0,halfPi);
}
const vehicleClasses=['scout_buggy','patrol4x4','apc','cargo6x6','mbt'];
const teamPaint={
  blue:{patrolKhaki:'slateBody',convoy:'slateBody',composite:'slateBody',convoyShade:'petrol',compositeEdge:'slateArmor',edgeTan:'slateArmor',freight:'freight',freightEdge:'slateArmor'},
  red:{patrolKhaki:'coyote',convoy:'clay',composite:'coyote',convoyShade:'clay',compositeEdge:'dune',edgeTan:'dune',freight:'coyote',freightEdge:'dune'}
};
// Adapt palette at collection time: all geometry remains immutable on the GPU in play.
function factionVehicle(r,kind,team){
  const paint=teamPaint[team],base=objects[kind],blue=team==='blue';
  const shell=Object.create(r);
  shell._material=key=>r._material(paint[key]||key);
  base(shell,true,team);
  if(kind.endsWith('_barrel'))return;
  if(vehicleClasses.includes(kind))serviceEquipment(shell,kind,blue);
  const ink=blue?'fieldBlue':'fieldRed', pale=blue?'signalBlue':'edgeTan';
  if(kind==='apc_turret'){
    if(blue){panel(r,-.28,.41,-.04,.19,.24,.33,'slateArmor');panel(r,-.28,.44,.144,.14,.10,.035,'optic');}
    else{r.cylinder(-.26,.30,-.19,.13,.105,.32,'clay',8);panel(r,.25,.45,-.14,.166,.13,.23,'dune');}
    return;
  }
  if(kind==='mbt_turret'){
    for(const side of [-1,1]){
      if(blue){for(const z of [-.66,-.22,.22])panel(r,side*1.108,.23,z,.14,.30,.385,'slateArmor',side*.03,0,side*.14);}
      else{for(const z of [-.86,-.59,-.31])panel(r,side*1.08,.25,z,.11,.48,.12,'dune',0,0,-side*.12);
        r.beam(side*1.15,.49,-.98,side*1.15,.49,-.17,.055,.055,'graphite');}
      panel(r,side*1.154,.26,.21,.044,.27,.23,ink);
    }
    if(blue){panel(r,.39,.65,-.39,.44,.30,.46,'petrol');panel(r,.39,.67,-.14,.30,.13,.036,'signalBlue');}
    else{r.cylinder(-.32,.61,-.60,.134,.103,.39,'coyote',8);r.beam(.76,.38,-.84,.91,1.25,-.94,.025,.025,'graphite');}
    return;
  }
  const dimensions={scout_buggy:[.70,1.65,-.65,.83],patrol4x4:[.84,1.85,-.97,1.0],cargo6x6:[1.07,2.47,-2.54,1.58],apc:[1.21,1.98,-1.73,1.49],mbt:[1.45,1.56,-1.90,1.09]}[kind];
  const [x,roof,rear,belt]=dimensions;
  for(const side of [-1,1]){
    const plane=side*(x+.045);
    if(blue){
      // Blue's flat upper rails and twin chevrons stay legible in profile.
      for(let i=0;i<2;i++)panel(r,plane,belt+.039,rear+.31+i*.25,.06,.23,.095,pale,0,.37);
      panel(r,plane,belt+.18,rear+.47,.054,.060,.54,ink);
    }else{
      // Red's slatted side baskets project past the hull.
      for(let i=0;i<3;i++)panel(r,side*(x+.077),belt-.035+i*.103,rear+.42,.068,.049,.83,'clay');
      for(const z of [rear+.035,rear+.8])panel(r,plane,belt+.065,z,.10,.36,.051,'graphite');
      panel(r,side*(x+.13),belt+.065,rear+.40,.025,.18,.24,ink);
    }
  }
  if(kind==='mbt'){
    if(blue){
      for(const sign of [-1,1]){
        panel(r,sign*.69,1.13,2.53,1.34,.38,1.01,'slateArmor',sign*.085,.30);
        panel(r,sign*1.37,.82,-1.71,.27,.53,.77,'slateArmor',0,-.14,sign*.14);
      }
    }else{
      for(const sign of [-1,1]){
        r.cylinder(sign*.71,1.62,-2.33,.238,.238,1.04,'coyote',12,0,0,halfPi);
        for(const z of [-2.33,-2.05])panel(r,sign*1.16,1.69,z,.13,.18,.09,'graphite');
      }
    }
    return;
  }
  if(kind==='apc'){
    if(blue){for(const side of [-1,1])panel(r,side*1.21,1.53,-.57,.30,.37,2.83,'slateArmor',0,0,-side*.20);}
    else{panel(r,0,2.12,-1.43,1.39,.34,1.37,'gaiter',0,.08);
      for(const z of [-1.89,-1.04])panel(r,0,2.13,z,1.42,.354,.072,'clay',0,.08);}
  }
  const cx=kind==='cargo6x6'?0:kind==='apc'?-.44:0,cz=kind==='cargo6x6'?-1.9:kind==='apc'?-1.46:-.23;
  if(blue){
    panel(r,cx,roof+.048,cz,.91,.07,.61,'petrol');panel(r,cx,roof+.116,cz,.79,.084,.48,'slateArmor');
    for(const side of [-1,1])panel(r,cx+side*.46,roof+.135,cz,.036,.105,.59,'graphite');
  }else if(kind!=='cargo6x6'){
    r.cylinder(cx-.18,roof+.13,cz,.149,.149,.77,'gaiter',8,0,0,halfPi);
    for(const dx of [-.43,.08])r.cylinder(cx+dx,roof+.13,cz,.154,.154,.046,'edgeTan',8,0,0,halfPi);
    r.beam(cx+.38,roof,cz,cx+.48,roof+.56,cz-.13,.027,.027,'graphite');
  }
}
// Bake the equipment with its chassis. Silhouette details add no runtime submissions.
function serviceEquipment(r,kind,blue){
  const trim=blue?'compositeEdge':'edgeTan';
  if(kind==='scout_buggy'){
    for(const side of [-1,1]){
      // Raised lamps and protective hoops belong to the open trail chassis.
      r.beam(side*.65,.68,1.53,side*.65,.96,1.58,.054,.055,'graphite');
      r.beam(side*.65,.96,1.58,side*.48,1.03,1.58,.048,.051,trim);
      r.beam(side*.49,1.66,-.68,side*.49,1.09,-1.34,.053,.058,'convoyShade');
      r.beam(side*.66,.89,.28,side*.48,.72,-.64,.043,.046,'edgeTan');
    }
    r.cylinder(0,.80,1.53,.107,.107,.49,'industrial',12,0,0,halfPi);
    for(const x of [-.25,.25])r.cylinder(x,.80,1.53,.14,.14,.043,'graphite',10,0,0,halfPi);
    panel(r,0,.74,1.64,.50,.068,.040,trim);
  }else if(kind==='patrol4x4'){
    for(const s of [-1,1]){
      r.beam(s*.54,.65,1.91,s*.54,1.09,1.92,.055,.062,'graphite');
      r.beam(s*.54,1.09,1.92,s*.82,1.06,1.86,.04,.044,'graphite');
      r.beam(s*.54,1.00,1.92,s*.82,.97,1.86,.032,.033,trim);
      r.beam(s*.48,1.26,.80,s*.22,1.45,.73,.018,.017,'underbody');
      r.beam(s*.69,1.87,-1.08,s*.69,1.99,-1.08,.041,.041,'graphite');
      r.beam(s*.69,1.99,-1.08,s*.69,1.99,.18,.041,.041,'graphite');
      r.beam(s*.69,1.99,.18,s*.69,1.87,.18,.041,.041,'graphite');
    }
    for(const z of [-1.04,-.45,.15])panel(r,0,1.98,z,1.41,.043,.052,trim);
  }else if(kind==='cargo6x6'){
    // Offset ladder, grab handles and a downward exhaust line on the cab.
    for(const x of [-.90,-.52])r.beam(x,1.02,-3.02,x,2.51,-3.02,.046,.053,'graphite');
    for(const y of [1.11,1.38,1.65,1.92,2.19,2.46])panel(r,-.71,y,-3.035,.40,.039,.052,'industrial');
    r.cylinder(.98,1.70,.56,.103,.103,1.21,'graphite',10);
    r.cylinder(.98,2.43,.56,.069,.062,.42,'industrial',10,0,0,-.14);
    for(const y of [1.37,1.99])r.cylinder(.98,y,.56,.115,.115,.068,trim,10);
    for(const s of [-1,1])r.beam(s*.85,1.82,2.321,s*.42,2.01,2.285,.025,.021,'graphite');
  }else if(kind==='apc'){
    // The ramp hinges interlock with the rear face, the perimeter rail stands off the roof.
    for(const s of [-1,1]){
      r.beam(s*.94,2.05,-2.11,s*.94,2.26,-2.11,.046,.046,'graphite');
      r.beam(s*.94,2.26,-2.11,s*.94,2.26,-.49,.045,.051,trim);
      r.beam(s*.94,2.26,-.49,s*.94,2.05,-.49,.046,.046,'graphite');
      for(const x of [.22,.67])r.cylinder(s*x,1.01,-2.92,.07,.07,.23,'industrial',8,0,0,halfPi);
      r.beam(s*.09,1.67,-2.925,s*.70,1.15,-2.925,.055,.037,trim);
    }
  }else{
    for(const s of [-1,1]){
      // Curved tow loop and straps below the front glacis.
      r.beam(s*.65,.89,2.86,s*.87,.69,2.87,.054,.054,'industrial');
      r.beam(s*.87,.69,2.87,s*1.03,.83,2.84,.054,.054,'industrial');
      r.beam(s*1.03,.83,2.84,s*1.08,1.08,2.63,.054,.054,'industrial');
      for(const z of [-2.16,-1.51])panel(r,s*.99,1.59,z,.28,.043,.077,trim);
    }
  }
}
function roadwheel(r,kind){
  const radius={patrol4x4:.431,cargo6x6:.523,apc:.523,scout_buggy:.426,mbt:.336}[kind];
  if(kind==='mbt'){
    r.cylinder(0,0,0,radius,radius,.10,'graphite',16,0,0,halfPi);
    r.cylinder(.063,0,0,.249,.249,.052,'convoy',12,0,0,halfPi);
    for(let i=0;i<6;i++){
      const a=i*Math.PI/3;
      panel(r,.101,.119*Math.sin(a),.119*Math.cos(a),.032,.14,.049,'industrial',0,halfPi-a);
    }
    r.cylinder(.121,0,0,.083,.083,.049,'graphite',8,0,0,halfPi);
    return;
  }
  wheel(r,0,0,0,radius,radius*.62,1);
  // Molded tire lugs are asymmetrical, making rotation visible across gravel.
  for(let k=0;k<12;k++){
    const a=k*Math.PI/6;
    panel(r,.01,(radius-.004)*Math.cos(a),(radius-.004)*Math.sin(a),radius*.57,.030,radius*.18,'underbody',0,a,-.12);
  }
  panel(r,radius*.31+.095,0,radius*.321,.019,.036,radius*.19,'edgeTan');
}
function trackBelt(r,frame){
  const length=4.26,rad=.324,total=2*length+2*Math.PI*rad,n=42;
  for(let j=0;j<n;j++){
    let t=(j+frame/12)*total/n, y,z,theta;
    if(t<length){z=-length/2+t;y=.50+rad;theta=0;}
    else if((t-=length)<Math.PI*rad){const a=t/rad;z=length/2+Math.sin(a)*rad;y=.50+Math.cos(a)*rad;theta=a;}
    else if((t-=Math.PI*rad)<length){z=length/2-t;y=.50-rad;theta=Math.PI;}
    else{const a=(t-length)/rad;z=-length/2-Math.sin(a)*rad;y=.50-Math.cos(a)*rad;theta=Math.PI+a;}
    panel(r,0,y,z,.43,.072,.203,'graphite',0,theta);
    panel(r,0,y+.045*Math.cos(theta),z+.045*Math.sin(theta),.40,.039,.049,'industrial',0,theta);
  }
}
function pickup(r,resource){
  const stripe={wood:'edgeTan',metal:'industrial',oil:'amber',fuel:'fieldRed',powder:'railYellow',ammo:'fieldBlue'}[resource]||'railYellow';
  if(resource==='wood'){
    for(let i=0;i<3;i++){r.cylinder((i-1)*.135,.121+(i===1?.093:0),0,.091,.091,.76,'wood',8,0,halfPi);}
    for(const z of [-.21,.21])panel(r,0,.135,z,.52,.24,.034,'convoyShade');
  }else if(resource==='metal'){
    for(let i=0;i<3;i++){panel(r,0,.06+i*.065,(i-1)*.068,.59,.049,.33,'industrial',i*.10);}
    panel(r,-.13,.128,0,.041,.19,.46,'graphite');
  }else if(resource==='oil'||resource==='fuel'){
    r.box(0,.235,0,.32,.47,.22,resource==='oil'?'convoyShade':'clay');
    r.box(0,.52,0,.22,.087,.077,'graphite');r.cylinder(-.095,.504,0,.05,.05,.055,stripe,8);
    for(const z of [-.115,.115]){panel(r,0,.25,z,.18,.12,.02,stripe,0,0,.59);panel(r,0,.25,z,.18,.12,.02,stripe,0,0,-.59);}
  }else{
    panel(r,0,.13,0,.50,.26,.35,'graphite'); panel(r,0,.266,0,.51,.05,.36,stripe);
    for(const x of [-.15,.15])panel(r,x,.134,0,.042,.29,.36,'industrial');
  }
}
function refineryHardware(r){
  panel(r,1.19,.945,.64,.48,.71,.22,'graphite');
  panel(r,1.19,.978,.768,.39,.56,.043,'industrial');
  r.cylinder(.49,.766,1.05,.29,.29,.18,'graphite',16,0,halfPi);
  for(const s of [-1,1])r.beam(.49+s*.23,.30,1.05,.49+s*.16,.67,1.05,.068,.068,'industrial');
  for(let j=0;j<4;j++)panel(r,1.19,.79+j*.076,.801,.31,.042,.027,'convoyShade');
}
function refineryLamp(r,mode){
  const color={idle:'standbyLamp',working:'runningLamp',ready:'readyLamp'}[mode];
  r.cylinder(0,.061,0,.105,.10,.122,'graphite',12);
  r.cylinder(0,.176,0,.09,.073,.117,color,12);
  r.sphere(0,.233,0,.145,.045,.145,color);
}
function refineryFan(r){
  for(let i=0;i<4;i++){const a=i*Math.PI*.5+.22;panel(r,.117*Math.cos(a),.117*Math.sin(a),0,.185,.072,.032,'railYellow',0,0,a);}
  r.cylinder(0,0,.025,.082,.082,.047,'industrial',8,0,halfPi);
}
module.exports={materials,objects,streetLamp,telecom,wreck,shippingContainer,tankBattery,factionVehicle,roadwheel,trackBelt,vehicleClasses,pickup,refineryHardware,refineryLamp,refineryFan};
