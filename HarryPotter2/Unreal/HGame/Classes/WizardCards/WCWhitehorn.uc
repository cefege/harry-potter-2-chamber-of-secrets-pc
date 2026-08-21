//===============================================================================
// #44 Devlin Whitehorn
//===============================================================================

class  WCWhitehorn extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardWhitehornTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Whitehornsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWhitehornBigTexture FILE=TEXTURES\menu\Folio\Cards\Whitehornbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Devlin Whitehorn";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=44
    skin(0)=Texture'HProps.skins.WizardCardWhitehornTex0'
	textureBig=Texture'WizCardWhitehornBigTexture'
	strDescriptionId="WizCard_0082"
}
