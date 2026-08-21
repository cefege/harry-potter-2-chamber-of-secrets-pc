//===============================================================================
//  #96 Hengist of Woodcroft 
//===============================================================================

class  WCWoodcroft extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardWoodcroftTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Woodcroftsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWoodcroftBigTexture FILE=TEXTURES\menu\Folio\Cards\Woodcroftbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Hengist of Woodcroft";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=96
    skin(0)=Texture'HProps.skins.WizardCardWoodcroftTex0'
	textureBig=Texture'WizCardWoodcroftBigTexture'
	strDescriptionId="WizCard_0009"
}
