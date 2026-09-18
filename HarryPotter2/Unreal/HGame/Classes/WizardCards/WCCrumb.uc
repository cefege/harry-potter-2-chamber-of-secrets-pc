//===============================================================================
// #56	Gideon Crumb
//===============================================================================

class  WCCrumb extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardCrumbTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Crumbsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardCrumbBigTexture FILE=TEXTURES\menu\Folio\Cards\Crumbbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Gideon Crumb";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=56
    skin(0)=Texture'HProps.skins.WizardCardCrumbTex0'
	textureBig=Texture'WizCardCrumbBigTexture'
	strDescriptionId="WizCard_0092"
}
