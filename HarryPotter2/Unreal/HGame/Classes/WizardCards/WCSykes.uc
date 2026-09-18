//===============================================================================
// #64 Jocunda Sykes
//===============================================================================

class  WCSykes extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardSykesTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Sykessmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardSykesBigTexture FILE=TEXTURES\menu\Folio\Cards\Sykesbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Jocunda Sykes";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=64
    skin(0)=Texture'HProps.skins.WizardCardSykesTex0'
	textureBig=Texture'WizCardSykesBigTexture'
	strDescriptionId="WizCard_0067"
}
