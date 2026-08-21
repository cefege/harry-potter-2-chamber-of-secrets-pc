class HPMessageBox expands UWindowDialogClientWindow;

#EXEC TEXTURE IMPORT NAME=FEMessageBoxBg	 FILE=TEXTURES\Menu\ConfirmDialog\confirmload.bmp GROUP="Icons" FLAGS=1 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEMessageBoxBtn	 FILE=TEXTURES\Menu\ConfirmDialog\confirmover.bmp GROUP="Icons" FLAGS=1 MIPS=OFF

var string Result;

var UWindowButton button1, button2;
var UWindowWrappedTextArea message;

var bool bClosing, bClosedFromTick;

var int FrameCount;
var float TimeOutTime, TimeOut;


function Setup (string set_message, string set_button1, optional string set_button2, optional float setTimeOut)
{
	message=UWindowWrappedTextArea(CreateControl(class'UWindowWrappedTextArea', 26,27, 220-26, 75-27));
	message.Clear();
	message.TextColor.r=215;
	message.TextColor.g=0;
	message.TextColor.b=215;
	message.Align=TA_Center;
	message.AddText(set_message);

	TimeOut  = setTimeOut;
	TimeOutTime = 0;
	FrameCount = 0;

	if (set_button2 != "")
	{
		button1=UWindowButton(CreateControl(class'UWindowButton', 32,87,61,15));
		button1.DownTexture=Texture'FEMessageBoxBtn';
		button1.OverTexture=Texture'FEMessageBoxBtn';
		button1.SetText(set_button1);
		button1.Align=TA_Center;
	}

	button2=UWindowButton(CreateControl(class'UWindowButton', 153,87,61,15));
	button2.DownTexture=Texture'FEMessageBoxBtn';
	button2.OverTexture=Texture'FEMessageBoxBtn';
	if (set_button2 != "")
		button2.SetText(set_button2);
	else
		button2.SetText(set_button1);
	button2.Align=TA_Center;

	log("HPMessageBox Setup message="$message.text$", timeOut="$timeOut);
}




function ScaleAndDraw(Canvas canvas,float x,float y,Texture tex)
{
local float fx,fy;

	fx=(canvas.SizeX/640.0);
	fy=(canvas.SizeY/480.0);
fx=1;fy=1;

	DrawStretchedTexture( canvas, x*fx, y*fy, tex.USize*fx,tex.VSize*fy,tex);

}


function Paint(Canvas canvas,float x,float y)
{
	ScaleAndDraw(canvas,0,0,Texture'FEMessageBoxBg');
}

function AfterPaint(Canvas C, float X, float Y)
{
	Super.AfterPaint(C, X, Y);

	if(TimeOut != 0)
	{
		FrameCount++;
		
		if(FrameCount >= 5)
		{
			TimeOutTime = GetEntryLevel().TimeSeconds + TimeOut;
			TimeOut = 0;
		}
	}

	if(TimeOutTime != 0 && GetEntryLevel().TimeSeconds > TimeOutTime)
	{
		TimeOutTime = 0;
		bClosedFromTick = true;
		Close();
	}
}



function Notify(UWindowDialogControl C, byte E)
{
	local int i;

	Super.Notify(C, E);

	if (C == None)
		return;

	switch(E)
	{
	case DE_Click:
		switch(C)
		{
		case button1:
		case button2:
			Result =C.text;
			log("HPMessageBox button clicked"@ Result);
			Close ();
			break;
		}
	}
}


function Close(optional bool bByParent)
{
	if (!bClosing)
	{
		log("HPMessageBox Close"@ message.text$", Result="@ Result);
		bClosing = true; // breaks a problem with recursion of things closing

		Super.Close(bByParent);
		OwnerWindow.WindowDone(Self);
	}
}

