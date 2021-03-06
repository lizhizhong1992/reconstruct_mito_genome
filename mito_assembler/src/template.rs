pub const TEMPLATE: &str = r#"
<!DOCTYPE html>
<meta charset="utf-8">
<html>
  <head>
    <script src="https://d3js.org/d3.v5.min.js"></script>
    <link rel="stylesheet" type="text/css" href="/viewer/style.css">
  </head>
  <body>
    <div class = "figure">
      <div id = "plot"></div>
      <div id = "info"></div>
    </div>
    <div id = "cr-info">
    </div>
    <script src="/viewer/circos.js"></script>
    <script> 
      const dataset = "/viewer/data.json";
      const repeats = "/viewer/repeats.json";
      const unit_length = 100;
      plotData(dataset,repeats,unit_length);
    </script>
  </body>
</html>
"#;

pub const TEMPLATE_LINEAR: &str = r#"<!DOCTYPE html>
<meta charset="utf-8">
<html>
  <head>
    <script src="https://d3js.org/d3.v5.min.js"></script>
    <link rel="stylesheet" type="text/css" href="/viewer/style.css">
  </head>
  <body>
    <div class ="figure">
        <div id = "plot"></div>
        <div id = "info"></div>
    </div>
    <script src="/viewer/linear.js"></script>
    <script>
      const dataset = "/viewer/read_data.json";
      const repeats = "/viewer/contig_alns.json";
      const unit_length = 100;
      plotData(dataset,repeats,unit_length);
    </script>
  </body>
</html>
"#;

pub const STYLE: &str = r#"
*{
    font-size: 20px;
}

.tick text{
    font-size: 15px;
}

.title{
    font-family: sans-serif;
    font-size: large;
}

body {
    display: flex;
    justify-content: flex-start;
}

.scale{
    font-size: 20px;
    font-family: sans-serif;
}

.scale text{
    font-size: 20px;
    font-size: medium;
}

.tick{
    stroke-width:1;
}

.contig{
}

.repeats{
    opacity:0.4;
}


.numofgapread{
    font-family: sans-serif;
    font-size: medium;
}

.critical-region{
    border-bottom: thin solid gray;
    margin: 10px;
}

.cluster-parent{
    border-style: solid;
    border-radius: 8px;
    border-width: 2px;
    padding: 10px;
}

.info-tip-clustercolor{
    display: flex;
    justify-content: center;
    margin: 10px;
}

.tooltip {
    position: absolute;			
    text-align: left;
    padding: 2px;				
    font: 14px sans-serif;
    font-weight: bold;
    border-color: gray;
    border-style: solid;
    border-radius: 8px;
    border-width: 2px;
    pointer-events: none;
}

#cr-info ul{
    margin: 0 0 5px 0;
}
"#;
