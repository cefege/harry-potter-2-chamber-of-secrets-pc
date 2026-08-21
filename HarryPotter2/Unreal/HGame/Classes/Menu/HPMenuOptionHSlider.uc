class HPMenuOptionHSlider extends UWindowHSliderControl;

#EXEC TEXTURE IMPORT NAME=FESliderTexture		 FILE=TEXTURES\Menu\Options\bar.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOverSliderTexture	 FILE=TEXTURES\Menu\Options\barHighlight.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FESliderKnobTexture	 FILE=TEXTURES\Menu\Options\slider.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

var Texture Image, overImage, knobImage;

var float fSliderOffsetX;

function Created ()
{
	Super.Created ();

	SliderWidth		= 134;
	
	Image			= Texture'FESliderTexture';
	overImage		= Texture'FEOverSliderTexture';
	knobImage		= Texture'FESliderKnobTexture';

	WinHeight		= 28;
	fSliderOffsetX	= 10;

	TrackWidth		= 9+26;

	//log("HPMenuOptionHSlider WinHeight"@ WinHeight);
}


function BeforePaint(Canvas C, float X, float Y)
{
	local float W, H;

//	Super.BeforePaint(C, X, Y);
	
	TextSize(C, Text, W, H);
	
	SliderDrawX = fSliderOffsetX + (WinWidth - SliderWidth);
	TextX		= SliderDrawX - W - 23;

	SliderDrawY = (WinHeight - 2) / 2;
	TextY		= (WinHeight - H) / 2;

	TrackStart = SliderDrawX + (SliderWidth - TrackWidth) * ((Value - MinValue)/(MaxValue - MinValue));
}

function Paint(Canvas C, float X, float Y)
{
	local Texture T;
	local Region R;

	T = GetLookAndFeelTexture();


	if(Text != "")
	{
		C.DrawColor = TextColor;
		ClipText(C, TextX, TextY, Text);
		C.DrawColor.R = 255;
		C.DrawColor.G = 255;
		C.DrawColor.B = 255;
	}
	
	R = LookAndFeel.HLine;
	
	DrawClippedTexture( C, SliderDrawX-fSliderOffsetX, 0, Image );
	
	if( MouseIsOver() )
		DrawClippedTexture( C, SliderDrawX-fSliderOffsetX, 0, overImage);

	DrawClippedTexture( C, TrackStart, 0, knobImage);
}
