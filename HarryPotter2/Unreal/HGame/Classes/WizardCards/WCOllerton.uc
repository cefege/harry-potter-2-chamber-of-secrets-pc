//===============================================================================
//  #57 Gifford Ollerton
//===============================================================================

class  WCOllerton extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardOllertonTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Ollertonsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardOllertonBigTexture FILE=TEXTURES\menu\Folio\Cards\Ollertonbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Gifford Ollerton";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=57
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE150"
    skin(0)=Texture'HProps.skins.WizardCardOllertonTex0'
	textureBig=Texture'WizCardOllertonBigTexture'
	strDescriptionId="WizCard_0093"
}
