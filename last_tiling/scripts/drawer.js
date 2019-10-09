const width = 1000;
const height = 1000;
// Margin in radian
const theta_margin = 0.15;
const gap_position = 0.05;
// the height of a contigs.
const contig_thick = 10; 
const coverage_thick = 5;
const gap_jitters = d3.randomNormal(0,0.01);
const read_thick = 4;
const eplison = 5.001;
const jitters = d3.randomNormal(0,eplison);

// Radius
const contig_radius = 350;
const coverage_min = contig_radius+contig_thick;
const coverage_max = 450;
const handle_points_radius = 100;
const read_radius = contig_radius-30;
const gap_min_radius = read_radius;
const gap_max_radius = contig_radius-3;
const gap_min = 1000;
const gap_max = 5000;
const gap_scale = d3.scaleLog()
      .domain([gap_min,gap_max])
      .range([gap_min_radius, gap_max_radius])
      .clamp(true);

const svg = d3.select("#plot")
      .append("svg")
      .attr("width",width)
      .attr("height",height);
const contigs_layer = svg.append("g")
      .attr("transform", `translate(${width/2},${height/2})`)
      .attr("class","contigs");
const coverage_layer = svg.append("g")
      .attr("transform", `translate(${width/2},${height/2})`)
      .attr("class","coverages");
const read_layer = svg.append("g")
      .attr("transform", `translate(${width/2},${height/2})`)
      .attr("class","read");
const info = d3.select("#info");

const calcScale = (contigs) => {
    // Input: Array of JSON object
    // Output: d3 Scale object
    // Requirements: each input object should have "length" attribute
    // Convert base pair into radian
    const num = contigs.length;
    const total = contigs.map(c => c.length).reduce((x,y)=>x+y);
    return d3.scaleLinear()
        .domain([0,total])
        .range([0,2 * Math.PI - num * theta_margin]);
};

const calcStartPosition = (contigs)=>{
    // Input: Array of JSON object.
    // Output: Array[Num]
    // Requirements: each input object should have "length" attribute
    // Map from the index(id) of contig into the start position of the contig(in radian).
    const scale = calcScale(contigs);
    const max = Math.max(...contigs.map(c => c.id));
    let cum_sum = 0;
    let start_pos = new Array(max);
    for (const contig of contigs){
        start_pos[contig.id] = scale(cum_sum) + contig.id * theta_margin;
        cum_sum += contig.length;
    }
    return start_pos;
};

const calcHandlePoints = (start_pos) => {
    // Input: Array[Num]
    // Output: Array[Array[Num]]
    // Requirement: None
    // Map from combinations of ID to the handle points of them.
    let handle_points = new Array();
    for (let i = 0 ; i < start_pos.length; i ++){
        handle_points.push(new Array(start_pos.length));
    }
    const max = start_pos.length-1;
    start_pos.forEach((v1,k1)=>{
        start_pos.forEach((v2,k2)=>{
            const next1 = (k1 == max ? Math.PI * 2 - theta_margin : start_pos[k1]);
            const next2 = (k2 == max ? Math.PI * 2 - theta_margin : start_pos[k2]);
            const val = (next1 + next2 + v1 + v2)/4 - theta_margin/2;
            handle_points[k1][k2] = val;
        });
    });
    return handle_points;
};

const calcCovScale = (contigs)=>{
    // Input: Array on JSON object
    // Output: d3.scale object
    // Requirements: each input object should have "length" attribute
    // Scale for convert coverage into radius.
    const max = Math.max(...contigs.map(contig => Math.max(...contig.coverages)));
    // const min = Math.min(...contigs.map(contig => Math.min(...contig.coverages)));
    return d3.scaleLinear()
        .domain([0,max])
        .range([coverage_min,coverage_max]);
};

const readToPath = (read,handle_points,bp_scale,start_pos,unit_length)=>{
    // Input: JSON object, Array[Array[Num]], d3.scale, Array[Num], Num
    // Output: String
    // Requirements: read should have units attribute, each of which elements
    // should have either "Gap" or "Encode"
    let path = d3.path();
    let units = Array.from(read.units).reverse();
    const r = read_radius + jitters();
    let gap = 0;
    let unit = {};
    while(!unit.hasOwnProperty("Encode")){
        unit = units.pop();
        if (unit == undefined){
            return "";
        }else if (unit.hasOwnProperty("Gap")){
            gap = unit.Gap;
        }
    };
    // Current ID of the contig 
    let contig = unit.Encode[0];
    let start = start_pos[contig] - Math.PI/2;
    let radian = start + bp_scale(unit_length*unit.Encode[1]);
    if (gap != 0){
        path.moveTo(gap_scale(gap) * Math.cos(radian), gap_scale(gap)*Math.sin(radian));
        path.lineTo(read_radius * Math.cos(radian), read_radius * Math.sin(radian));
    }else{
        path.moveTo(read_radius * Math.cos(radian), read_radius * Math.sin(radian));
    }
    gap = 0;
    for (unit of units.reverse()){
        if (unit.hasOwnProperty("Gap")){
            if (unit.Gap > unit_length * 2){
                gap = unit.Gap;
            }
            continue;
        }
        if (unit.Encode[0] == contig){
            radian = start + bp_scale(unit_length*unit.Encode[1]);
            path.lineTo(r * Math.cos(radian), r*Math.sin(radian));
        }else{
            // Change contig. Connect them.
            // If there are remaining gap, clean them.
            if (gap != 0){
                const control_radian = start_pos[contig] - Math.PI/2;
                const new_radian = control_radian - gap_position;
                const control_x = handle_points_radius * Math.cos(control_radian);
                const control_y = handle_points_radius * Math.sin(control_radian);
                const jt = gap_jitters();
                path.quadraticCurveTo(control_x, control_y, r * Math.cos(new_radian), r * Math.sin(new_radian));
                path.moveTo(gap_scale(gap) * Math.cos(new_radian + jt), gap_scale(gap)*Math.sin(new_radian + jt));
                path.lineTo(r * Math.cos(new_radian), r * Math.sin(new_radian));
            }
            gap = 0;
            const new_radian = start_pos[unit.Encode[0]];
            radian = new_radian + bp_scale(unit_length*unit.Encode[1]) - Math.PI/2;
            // Bezier Curve to new point from here.
            const control_radius = handle_points[contig][unit.Encode[0]] - Math.PI/2;
            const control_x = handle_points_radius*Math.cos(control_radius);
            const control_y = handle_points_radius*Math.sin(control_radius);
            contig = unit.Encode[0];
            start = start_pos[contig] - Math.PI/2;
            path.quadraticCurveTo(control_x,control_y,r*Math.cos(radian),r*Math.sin(radian));
        }
    }
    return path.toString();
};

const calcID = (read,unit_length)=>{
    // Input: Json object
    // Output: Num
    // Requirements: read should have "units" property, which is a vector
    // and each of element should have eigther "Gap" or "Encode" type.
    // Returns the most assigned type of given read.
    const gap = read
          .units
          .filter(unit => unit.hasOwnProperty("Gap"))
          .reduce((g, unit) => g + unit.Gap,0);
    const summary = read
          .units
          .filter(unit => unit.hasOwnProperty("Encode"))
          .map(unit => unit.Encode[0])
          .reduce((map,ctg)=>{
              if (map.has(ctg)){
                  map.set(ctg,map.get(ctg)+unit_length);
              }else{
                  map.set(ctg,unit_length);
              }
              return map;
          }, new Map());
    let max = undefined;
    summary
        .forEach((len,ctg)=>{
            if (max == undefined || max.len < len){
                max = {"ctg":ctg, "len":len};
            }else {
                max = max;
            }});
    if (max == undefined){
        return {"type":"Gap"};
    }else{
        return (max.len < gap ? {"type":"Gap"} : {"type":"Contig", "id":max.ctg});
    }
};


const selectRead = read => {
    // Input: JSON object
    // Output: boolean
    // Requirements: input object should have "units" property,
    // which is actually vector of object with "Gap" or "Encode" property.
    // Filter read as you like.
    const from = 0;
    const to = 1;
    const set = new Set(read.units.filter(u => u.hasOwnProperty("Encode")).map(u => u.Encode[0]));
    const max_gap = Math.max(...read.units.filter(u => u.hasOwnProperty("Gap")).map(u => u.Gap));
    return true;
    // return set.has(4) && set.size == 1;
    // return set.has(from) && set.has(to) && read.units.length > 15 ;
    // return read.units.length < 140 && read.units.length > 75 && set.size > 1 && set.has(0) && set.has(1) && max_gap < 4000;
    // return set.size == 2 && set.has(0) && set.has(1); // && max_gap < 4000;
    // return set.size == 1 && set.has(1) ;
};

const getNumOfGapRead = reads => {
    // Input: [JSON object]
    // Output: Num
    // Requirements: each element should be 'read' object.
    // Return numbers of reads which is just Gap.
    return reads.filter(read => {
        let units = Array.from(read.units);
        let unit = {};
        while(!unit.hasOwnProperty("Encode")){
            unit = units.pop();
            if (unit == undefined){
                return true;
            }
        };
        return false;
    }).length;
};

const plotData = (dataset, repeats, unit_length) =>
      Promise.all([dataset, repeats]
                  .map(file => d3.json(file)))
      .then(([values, repeats]) => {
          // Unpack
          // This is array.
          const contigs = values.contigs;
          // This is also an array.
          // const reads = values.reads;
          // Or select reads as you like.
          const reads = values.reads.filter(selectRead);
          // let reads = values.reads.slice(0,10);
          // reads.push({"name":"test",
          //             "units":[{"Gap":1000},
          //                      {"Encode":[0,0]},{"Encode":[0,1]},{"Encode":[0,2]},{"Encode":[2,100]},
          //                      {"Gap":2000}]});
          // Calculate coordinate.
          const bp_scale = calcScale(contigs);
          const coverage_scale = calcCovScale(contigs);
          const start_pos = calcStartPosition(contigs);
          const handle_points = calcHandlePoints(start_pos);
          const contig_num = start_pos.length;
          // Draw contigs.
          contigs_layer
              .selectAll(".contig")
              .data(contigs)
              .enter()
              .append("path")
              .attr("class","contig")
              .attr("d", contig =>  {
                  const end = (contig.id == contig_num-1 ? Math.PI*2 : start_pos[contig.id+1]) - theta_margin;
                  const arc = d3.arc()
                        .innerRadius(contig_radius)
                        .outerRadius(contig_radius +  contig_thick)
                        .startAngle(start_pos[contig.id])
                        .endAngle(end);
                  return arc();
              })
              .attr("fill",c => d3.schemeCategory10[c.id% 10]);
          // Draw repeat.
          contigs_layer
              .selectAll(".repeats")
              .data(repeats.flatMap(rp => rp.reps))
              .enter()
              .append("path")
              .attr("class","repeats")
              .attr("d", repeat => {
                  const arc = d3.arc()
                        .innerRadius(contig_radius - 3)
                        .outerRadius(contig_radius + contig_thick + 3)
                        .startAngle(start_pos[repeat.id] + bp_scale(repeat.start))
                        .endAngle(start_pos[repeat.id] + bp_scale(repeat.end));
                  return arc();
              })
              .attr("fill", "gray");
          coverage_layer
              .selectAll(".coverage")
              .data(contigs)
              .enter()
              .append("path")
              .attr("class","coverage")
              .attr("d", contig => {
                  const start = start_pos[contig.id];
                  const arc = d3.lineRadial()
                        .angle((_,i) => start + bp_scale(i * unit_length))
                        .radius(d => coverage_scale(d));
                  return arc(contig.coverages);
              })
              .attr("fill","none")
              .attr("stroke",c => d3.schemeCategory10[c.id% 10]);
          // console.log(reads);
          // Draw reads
          read_layer
              .selectAll(".read")
              .data(reads)
              .enter()
              .append("path")
              .attr("class","read")
              .attr("d",read => readToPath(read,handle_points,bp_scale,start_pos,unit_length))
              .attr("fill","none")
              .attr("opacity",0.2)
              .attr("stroke",read => {
                  const identity = calcID(read,unit_length);
                  if (identity.type == "Gap"){
                      return "black";
                  }else{
                      return d3.schemeCategory10[identity.id % 10];
                  }
              });
          info.append("div")
              .attr("class","numofgapread")
              .append("p")
              .text(`Gap Read:${getNumOfGapRead(reads)} out of ${reads.length}`);
          // Draw ticks.
          const b_tick = svg.append("g")
                .attr("class","scale")
                .attr("transform",`translate(0,40)`);
          b_tick.append("text")
              .text("Base Pair Scale");
          {
              const bscale = d3.scaleLinear()
                    .domain([0,100000])
                    .range([contig_radius*bp_scale(0),contig_radius*bp_scale(100000)]);
              b_tick.append("g")
                  .attr("transform","translate(50,5)")
                  .call(d3.axisBottom(bscale)
                        .tickFormat(d3.format(".2s"))
                        .ticks(4));
          }
          const c_tick = svg.append("g")
                .attr("class","scale")
                .attr("transform",`translate(0,100)`);
          c_tick.append("text")
              .text("Coverage Scale");
          {
              const cscale = d3.scaleLinear()
                    .domain([0,1500])
                    .range([0,coverage_scale(1500)-coverage_scale(0)]);
              c_tick.append("g")
                  .attr("transform",`translate(50,5)`)
                  .call(d3.axisBottom(cscale)
                        .tickFormat(d3.format(".2s"))
                        .ticks(4));
          }
          const g_tick = svg.append("g")
                .attr("class","scale")
                .attr("transform",`translate(0,160)`);
          g_tick.append("text")
              .text("Gap Scale");
          {
              const gscale = d3.scaleLog()
                    .domain([gap_min,5*gap_max])
                    .range([0, 5*(gap_max_radius-gap_min_radius)]);
              g_tick.append("g")
                  .attr("transform",`translate(50,5)`)
                  .call(d3.axisBottom(gscale)
                        .tickFormat(d3.format(".2s"))
                        .ticks(1)
                       );
          }
          // const pic = document.getElementById("plot");
          //get svg source.
          // var serializer = new XMLSerializer();
          // var source = serializer.serializeToString(pic);
          // var svgBlob = new Blob([source], {type:"image/svg+xml;charset=utf-8"});
          // var svgUrl = URL.createObjectURL(svgBlob);
          // var downloadLink = document.createElement("a");
          // downloadLink.href = svgUrl;
          // downloadLink.download = "newesttree.svg";
          // document.body.appendChild(downloadLink);
          // downloadLink.click();
          // document.body.removeChild(downloadLink);
      })
      .then(ok => console.log("OK"),
            why => console.log(why));
