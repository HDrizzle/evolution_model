// MISC geometry stuff, by Hadrian Ward

class Vector
{
	constructor(x, y)
	{
		// assign properties
		this.x = x;
		this.y = y;
		this.v = [x, y];
		// calculate angle (deg)
		var angle = Math.abs(deg(Math.asin(this.v[1] / this.abs())))
		if(isNaN(angle)){angle = 90}
		if(x < 0){angle = 180 - angle}
		if(y < 0){angle = 360 - angle}
		this.angle = angle;
	}
	abs(){return Math.pow(Math.pow(this.x, 2) + Math.pow(this.y, 2), 0.5)}
	add(other)
	{
		return new Vector(this.x + other.x, this.y + other.y);
	}
	sub(other)
	{
		return new Vector(this.x - other.x, this.y - other.y);
	}
	mult(ratio)
	{
		return new Vector(this.x * ratio, this.y * ratio);
	}
	static fromAngle(angle, length)
	{
		var angleR = rad(angle);
		return new Vector(Math.cos(angleR), Math.sin(angleR)).mult(length);
	}
}

function deg(r)
{
	return ((r * 180) / Math.PI) % 360;
}

function rad(d)
{
	return (d * Math.PI) / 180;
}