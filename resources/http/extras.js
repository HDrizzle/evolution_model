// MISC JS functions / stuff that can be used by any page

function centerBody()// puts the <body> tag in the center of the screen
{
	var screenWidth = window.screen.width;
	var body = document.querySelector("body")
	var bodyWidth = body.offsetWidth;// no pun intended
	body.style = "margin-left: " + Math.max((screenWidth - bodyWidth) / 2, 5) + "px;";
}

function parseTimeString(timeStr)
{
	var arr = timeStr.split(":");
	var validLengths = [1, 2, 3];
	var multipliers = {1: [1], 2: [60, 1], 3: [3600, 60, 1]};
	if(validLengths.includes(arr.length))
	{
		var result = 0;
		for(i = 0; i < arr.length; i ++)
		{
			result += multipliers[arr.length][i] * arr[i];
		}
		return result;
	}
	else
	{
		return null;
	}
}

function formatTimeString(secs)// opposite of parseTimeString(), copied from https://stackoverflow.com/questions/1322732 (w/ some modification), answer by Harish Ambady
{
	var date = new Date(null);
	date.setSeconds(secs); // specify value for SECONDS here
	if(secs < 60)// < 1 minute
	{
		return date.toISOString().substr(17, 2);
	}
	if(secs < 3600)// < 1 hour
	{
		return date.toISOString().substr(14, 5);
	}
	return date.toISOString().substr(11, 8);// >= 1 hour
}

// from https://dirask.com/posts/JavaScript-convert-RGB-color-to-CSS-HEX-color-pqeZgp
const nToStringHex = (component) => {
    if (component < 0 || component > 255) {
        throw new Error('Incorrect color component value.');
    }
    const text = component.toString(16);
    if (component < 16) {
        return '0' + text;
    }
    return text;
};

const RGBToHex = (rgb) => '#' + nToStringHex(rgb[0]) + nToStringHex(rgb[1]) + nToStringHex(rgb[2]);

function formatStringMatrix(matrix, sep)// matrix: array (top down) of rows (left to right)
{
	// default seperator: " - "
	if(typeof(sep) === 'undefined') sep = " - ";
	// get column widths
	var columnWidths = [];
	for(x = 0; x < matrix[0].length; x++)
	{
		var currLengths = [];
		for(y = 0; y < matrix.length; y++)
		{
			currLengths.push(matrix[y][x].toString().length);
		}
		columnWidths.push(Math.max(...currLengths));
	}
	// create string
	var returnStr = "";
	for(y = 0; y < matrix.length; y++)
	{
		var x = 0;
		matrix[y].map(
			function(str)
			{
				if(x > 0) returnStr += sep;
				returnStr += str.toString().padStart(columnWidths[x], " ")
				x++;
			}
		)
		returnStr += "\n";// end of line
	}
	return returnStr;// Done
}

function asyncRequest(action, dir, sendString, contentType, doneCallback, statusElement, userMessages)
{
	if(typeof(userMessages) === 'undefined') userMessages = ["Updated", "Failed"];
	var xhr = new XMLHttpRequest();
	xhr.open(action, dir);
	xhr.setRequestHeader("Content-Type", contentType);
	xhr.onreadystatechange = function(){
		if(xhr.readyState === 4){
			doneCallback(xhr.status, xhr.responseText);
			if(xhr.status == 200)
			{
				statusElement.innerHTML = userMessages[0];
				statusElement.style.color = "#089500";
			}
			else
			{
				statusElement.innerHTML = userMessages[1] + ", HTTP code=" + xhr.status;
				statusElement.style.color = "#FF0000";
			}
		}};
	// waiting
	statusElement.innerHTML = "Waiting...";
	statusElement.style.color = "#000000";
	// send request
	xhr.send(sendString);
}

function timeStampToDateTime(ts)
{
	var date = new Date(ts * 1000);
	return date.getFullYear() + "-" + (date.getMonth() + 1) +
    	"-" + date.getDate() + " " + date.getHours().toString().padStart(2, '0') +
    	":" + date.getMinutes().toString().padStart(2, '0') + ":" + date.getSeconds().toString().padStart(2, '0');
}
