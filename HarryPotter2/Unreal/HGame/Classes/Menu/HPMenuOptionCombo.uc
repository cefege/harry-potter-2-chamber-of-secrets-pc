class HPMenuOptionCombo extends UWindowComboControl;

#EXEC TEXTURE IMPORT NAME=FEComboIdleTexture FILE=TEXTURES\Menu\Options\overoption5.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEComboOverTexture FILE=TEXTURES\Menu\Options\overoption.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

var Texture IdleTexture, OverTexture;

function Created()
{
	Super.Created();

	IdleTexture   = Texture'FEComboIdleTexture';
	OverTexture   = Texture'FEComboOverTexture';
	WinHeight = 24;
}

function CreateEditBox ()
{
	EditBox = HPMenuOptionEditBox(CreateWindow(class'HPMenuOptionEditBox', 0, 0, WinWidth, WinHeight)); 
}

function CreateComboButton ()
{
	Button = HPMenuOptionComboButton(CreateWindow(class'HPMenuOptionComboButton', WinWidth-12, 0, 12, 10)); 
}

function CreateComboList ()
{
	List = HPMenuOptionComboList(Root.CreateWindow(ListClass, 0, 0, 150, 58)); 
}

function BeforePaint(Canvas C, float X, float Y)
{
	local float W, H;

	Super.BeforePaint(C, X, Y);

	WinHeight = 24;

	TextSize(C, Text, W, H);

	TextY = (WinHeight - H) / 2;
	
	TextX = WinWidth - W - 20 - EditBoxWidth;
}


function Paint(Canvas C, float X, float Y)
{
	if(Text != "")
	{
		C.DrawColor = TextColor;
		ClipText(C, TextX, TextY, Text);
		C.DrawColor.R = 255;
		C.DrawColor.G = 255;
		C.DrawColor.B = 255;
	}

	if (bListVisible)
		DrawClippedTexture( C, WinWidth-EditBoxWidth, 1, OverTexture );
	else
	{
		if( MouseIsOver() || EditBox.MouseIsOver() || Button.MouseIsOver() )
		{
			DrawClippedTexture( C, WinWidth-EditBoxWidth, 1, OverTexture );
		}
		else
		{
			DrawClippedTexture( C, WinWidth-EditBoxWidth, 1, IdleTexture );
		}
	}
}

function CloseUpWithNoSound()
{
	bListVisible = False;
	EditBox.SetEditable(bCanEdit);
	EditBox.SelectAll();
	List.HideWindow();
}

defaultproperties
{
	ListClass=class'HPMenuOptionComboList'
}