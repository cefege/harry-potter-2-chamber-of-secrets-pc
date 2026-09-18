//=============================================================================
// ShortCutButton - Button 
//=============================================================================
class ShortCutButton expands UWindowButton;

#EXEC TEXTURE IMPORT NAME=ButtonUpTexture		FILE=TEXTURES\Menu\ICONS\buttonUp.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=ButtonDownTexture		FILE=TEXTURES\Menu\ICONS\buttonDown.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
//#EXEC TEXTURE IMPORT NAME=BlueOverTexture		FILE=TEXTURES\Menu\ICONS\BlueOver.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


var string		strText;

function Created()
{
	Super.Created();
	
	UpTexture	= Texture'ButtonUpTexture';
	DownTexture	= Texture'ButtonDownTexture';
	OverTexture	= Texture'ButtonUpTexture';
	
	strText = "NOSTR";
}

function SetText( string NewText )
{
	strText = NewText;
}


function Paint(Canvas canvas,float x,float y)
{
	Super.Paint( canvas, x, y );
	
	canvas.DrawColor.r = 0;
	canvas.DrawColor.g = 0;
	canvas.DrawColor.b = 0;
	
	// offset our text so that it is closer to the center of the button
	ClipText( canvas, 4 , 4, strText );

}
