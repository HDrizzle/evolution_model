// main JS file

// ------------------------------------ update document ------------------------------------

function startUpdateRequest()
{
	//asyncRequest(action, dir, sendString, contentType, doneCallback, statusElement, userMessages)
	asyncRequest
	(
		'GET',
		'/sim.json',
		'',
		'',
		function(code, data){if(code == 200){updateReceived(data)}},
		E("update-status")
	)
}

function updateReceived(dataStr)
{
	data = JSON.parse(dataStr)
	// deselect entity if died
	if(!Object.keys(data.entities).includes(selectedEntity)){selectedEntity = null}
	updateStats(data);
	renderEntities(data.entities);
	updateSelectedEntity();
	setTimeout(startUpdateRequest, 20);
}

function updateStats(data)
{
	// entities
	var entities = data.entities;
	// count entities of each type
	var typeCounts = Object();
	var orderedKeys = Object.keys(entities);
	for(i = 0; i < orderedKeys.length; i++)
	{
		var type = entities[orderedKeys[i]].type_
		if(Object.keys(typeCounts).includes(type))
		{
			typeCounts[type]++;
		}
		else
		{
			typeCounts[type] = 1;
		}
	}
	// display
	var e = E("entity-count");
	e.innerHTML = "";
	var orderedKeys = Object.keys(entityTypes);
	for(i = 0; i < orderedKeys.length; i++)
	{
		var type = orderedKeys[i]
		if(i > 0){e.innerHTML += ", "}
		e.innerHTML += type + ": " + typeCounts[type];
	}
	// timing
	e.innerHTML += ", Running time: " + formatTimeString(Math.round(data.t)) + ", Saved: " + timeStampToDateTime(data.t_saved);
	// server stats
	E("server-fps").innerHTML = Math.round(Number(data.fps) * 100) / 100;
	E("server-performance").innerHTML = data.performance;
}

function renderEntities(entities)
{
	// clear
	canvasCtx.clearRect(0, 0, canvas.width, canvas.height);
	var orderedKeys = Object.keys(entities);
	// loop through entities
	for(i = 0; i < orderedKeys.length; i++)
	{
		var e = entities[orderedKeys[i]];
		var pos = posToCanvas(e.pos);
		var color = entityTypes[e.type_].color;
		var radius = entityTypes[e.type_].radius * canvasScale;
		// body
		canvasCtx.beginPath();
		canvasCtx.arc(pos[0], pos[1], radius, 0, 2 * Math.PI, false);
		canvasCtx.strokeStyle = '#000000';
		canvasCtx.fillStyle = RGBToHex(color);
		canvasCtx.fill();
		// stroke
		canvasCtx.stroke();
		// eyes, version 2022-10-23
		var eye_offset_angle = 25;
		var eye_radius = radius / 4;
		for(var curr_offset_angle = -eye_offset_angle; curr_offset_angle < eye_offset_angle + 1; curr_offset_angle += eye_offset_angle * 2)
		{
			var eye_pos = new Vector(pos[0], pos[1]).add(Vector.fromAngle((-e.orientation) + curr_offset_angle, radius * 0.66));
			canvasCtx.beginPath();
			canvasCtx.arc(eye_pos.v[0], eye_pos.v[1], eye_radius, 0, 2 * Math.PI, false);
			canvasCtx.fillStyle = '#000000';
			canvasCtx.fill();
			canvasCtx.strokeStyle = '#FFFFFF';
			canvasCtx.stroke();
		}
	}
	if(selectedEntity)// entity is selected
	{
		var e = entities[selectedEntity];
		var pos = posToCanvas(e.pos);
		var radius = entityTypes[e.type_].radius * canvasScale;
		var visionAngleHalfDeg = entityTypes[e.type_].vision / 2;
		var visionAngleHalf = rad(visionAngleHalfDeg);
		var visionRange = entityTypes[e.type_].vision_range * canvasScale;
		var radAngle = rad(-e.orientation);
		// square highlight box
		canvasCtx.beginPath();
		canvasCtx.strokeStyle = '#0000FF';
		canvasCtx.rect(pos[0] - radius, pos[1] - radius, radius * 2, radius * 2);
		canvasCtx.stroke();
		// vision range arc
		entityVisionArcEdgeOverflow(pos, radAngle, visionAngleHalf, visionRange);
	}
}

function entityVisionArcEdgeOverflow(pos, angle, visionAngleHalf, visionRange)// everything going into this should be in radians or pixels
{
	var w = canvas.width;
	var h = canvas.height;
	var x = pos[0];
	var y = pos[1];
	// original one
	entityVisionArc(pos, angle, visionAngleHalf, visionRange);
	// combinations for each adjacent "tile", TODO get working
	entityVisionArc([x + w, y], angle, visionAngleHalf, visionRange);
	entityVisionArc([x + w, y + h], angle, visionAngleHalf, visionRange);
	entityVisionArc([x, y + h], angle, visionAngleHalf, visionRange);
	entityVisionArc([x - w, y + h], angle, visionAngleHalf, visionRange);
	entityVisionArc([x - w, y], angle, visionAngleHalf, visionRange);
	entityVisionArc([x - w, y - h], angle, visionAngleHalf, visionRange);
	entityVisionArc([x, y - h], angle, visionAngleHalf, visionRange);
	entityVisionArc([x + w, y - h], angle, visionAngleHalf, visionRange);
}

function entityVisionArc(pos, angle, visionAngleHalf, visionRange)
{
	// Arc
	canvasCtx.beginPath();
	canvasCtx.arc(pos[0], pos[1], visionRange, angle + visionAngleHalf, angle - visionAngleHalf, true);
	canvasCtx.stroke();
	// Sides of arc
	if (visionAngleHalf < Math.PI) {
		var posVect = new Vector(pos[0], pos[1]);
		canvasCtx.beginPath();
		var v = (posVect.add(Vector.fromAngle(deg(angle + visionAngleHalf), visionRange))).v;
		canvasCtx.moveTo(v[0], v[1]);
		canvasCtx.lineTo(pos[0], pos[1]);
		var v = (posVect.add(Vector.fromAngle(deg(angle - visionAngleHalf), visionRange))).v;
		canvasCtx.lineTo(v[0], v[1]);
		canvasCtx.stroke();
	}
}

function posToCanvas(pos)
{
	return [pos[0] * canvasScale, canvas.height - (pos[1] * canvasScale)]
}

// ------------------------------------ entity details ------------------------------------

function updateSelectedEntity()
{
	var entities = data.entities;
	var e = entities[selectedEntity];
	elem = E("entity-details");
	elem.innerHTML = "";
	if(e != null)
	{
		let matrix = [
			["Type", e.type_],
			["ID", selectedEntity],
			["Health", Math.round(e.health * 100) + "%"],
			["Fitness", Math.round(e.fitness)],
			["Generation", e.gen],
			["Energy copy", Math.round(e.energy_copy * 100) / 100],
			["Angle", Math.round(e.orientation)],
			["NN inputs:", ""]
		];
		let nn_inputs = e.latest_inputs;
		for (let i = 0; i < nn_inputs.length; i++)
		{
			matrix.push([i + "", (Math.round(nn_inputs[i] * 1000) / 1000) + " "]);
		}
		elem.innerHTML += formatStringMatrix(matrix);
	}
	else
	{
		elem.innerHTML += "Select entity\nfor details";
	}
}

function canvasClicked(e)
{
	// https://stackoverflow.com/questions/3234256
	var rect = e.target.getBoundingClientRect();
	var x = e.clientX - rect.left// x position within the element
	var y = e.clientY - rect.top;// y position within the element
	// select entity
	selectedEntity = null;
	selectedDist = null;
	var entities = data.entities;
	var orderedKeys = Object.keys(entities);
	for(i = 0; i < orderedKeys.length; i++)
	{
		var ID = orderedKeys[i];
		var e = entities[ID];
		var pos = posToCanvas(e.pos)
		var dist = new Vector(x, y).sub(new Vector(pos[0], pos[1])).abs()
		if (dist < selectedDist || selectedDist == null)// if this is the closest entity so far
		{
			selectedEntity = ID;
			selectedDist = dist;
		}
	}
	updateSelectedEntity();
}


// ------------------------------------ onload ------------------------------------

window.onload = function(){
	setCanvasSize();
	centerBody();
	canvas = E("main-canvas")
	// set canvas size
	/*var canvas_size = Math.min(window.screen.width, window.screen.height) + 'px';
	canvas.style.width = canvas_size;
	canvas.style.height = canvas_size;*/
	canvasCtx = canvas.getContext('2d');
	selectedEntity = null;// ID of selected entity
	// load entity_types.json
	//asyncRequest(action, dir, sendString, contentType, doneCallback, statusElement, userMessages)
	asyncRequest
	(
		'GET',
		'/settings.json',
		'',
		'',
		function(code, data)
		{
			if(code == 200)
			{
				window.settings = JSON.parse(data);
				window.entityTypes = window.settings.entity_types;
				startUpdateRequest();
				canvas.onclick = canvasClicked;
			}
		},
		E("entity-types-load-status"),
		["Loaded entity types", "ERROR: could not load entity_types, check browser console"]
	)
}

function setCanvasSize()
{
	canvasScale = 0.8// TODO
}
