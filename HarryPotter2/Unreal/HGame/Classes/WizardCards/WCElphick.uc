//===============================================================================
// #91 Wilfred Elphick
//===============================================================================

class  WCElphick extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardElphickTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Elphicksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardElphickBigTexture FILE=TEXTURES\menu\Folio\Cards\Elphickbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Wilfred Elphick";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=91
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE170"
    skin(0)=Texture'HProps.skins.WizardCardElphickTex0'
	textureBig=Texture'WizCardElphickBigTexture'
	strDescriptionId="WizCard_0005"
}
