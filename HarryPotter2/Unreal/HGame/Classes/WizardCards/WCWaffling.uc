//===============================================================================
//   #24 Adalbert Waffling
//===============================================================================

class  WCWaffling extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardWafflingTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Wafflingsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWafflingBigTexture FILE=TEXTURES\menu\Folio\Cards\Wafflingbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Adalbert Waffling";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=24
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE120"
    skin(0)=Texture'HProps.skins.WizardCardWafflingTex0'
	textureBig=Texture'WizCardWafflingBigTexture'
	strDescriptionId="WizCard_0020"
}
