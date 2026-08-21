//===============================================================================
// #27 Mirabella Plunkett
//===============================================================================

class  WCPlunkett extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardPlunkettTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Plunkettsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardPlunkettBigTexture FILE=TEXTURES\menu\Folio\Cards\Plunkettbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Mirabella Plunkett";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=27
    skin(0)=Texture'HProps.skins.WizardCardPlunkettTex0'
	textureBig=Texture'WizCardPlunkettBigTexture'
	strDescriptionId="WizCard_0023"
}
