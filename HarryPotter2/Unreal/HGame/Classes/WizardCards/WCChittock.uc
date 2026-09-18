//===============================================================================
// #23 Glenda Chittock
//===============================================================================

class  WCChittock extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardChittockTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Chittocksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardChittockBigTexture FILE=TEXTURES\menu\Folio\Cards\Chittockbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Glenda Chittock";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=23
    skin(0)=Texture'HProps.skins.WizardCardChittockTex0'
	textureBig=Texture'WizCardChittockBigTexture'
	strDescriptionId="WizCard_0019"
}
